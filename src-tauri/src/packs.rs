//! Sound packs: the `pack.json` format, validation, and loading packs into
//! decoded sound banks. Built-in packs are compiled into the binary; community
//! packs live in the user's local app data folder.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::keymap::KeyClass;

include!(concat!(env!("OUT_DIR"), "/builtin_sounds.rs"));

pub const DEFAULT_PACK: &str = "holy-panda";
pub const MANIFEST: &str = "pack.json";
pub const MAX_FILES: usize = 64;
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_PACK_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_CLIP_SECONDS: usize = 4;
const AUDIO_EXTENSIONS: [&str; 4] = ["wav", "mp3", "ogg", "flac"];
pub const ROWS: usize = 5;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SoundSet {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub space: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enter: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backspace: Vec<String>,
}

impl SoundSet {
    fn paths(&self) -> impl Iterator<Item = &String> {
        self.default
            .iter()
            .chain(self.rows.iter().flatten())
            .chain(&self.space)
            .chain(&self.enter)
            .chain(&self.backspace)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Credit {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub description: String,
    pub press: SoundSet,
    #[serde(default)]
    pub release: SoundSet,
    #[serde(default)]
    pub credits: Vec<Credit>,
}

impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err("pack.json is too large".into());
        }
        let manifest: Manifest =
            serde_json::from_slice(bytes).map_err(|e| format!("invalid pack.json: {e}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 {
            return Err(format!("unsupported pack schema {}", self.schema));
        }
        if !valid_id(&self.id) {
            return Err(format!("invalid pack id {:?}", self.id));
        }
        check_text("name", &self.name, 1, 64)?;
        check_text("version", &self.version, 0, 32)?;
        check_text("author", &self.author, 0, 200)?;
        check_text("license", &self.license, 0, 64)?;
        check_text("type", &self.kind, 0, 16)?;
        check_text("description", &self.description, 0, 300)?;
        if let Some(source) = &self.source {
            check_text("source", source, 0, 300)?;
        }
        if self.credits.len() > MAX_FILES {
            return Err("too many credits".into());
        }
        for credit in &self.credits {
            check_text("credit file", &credit.file, 0, 96)?;
            check_text("credit author", &credit.author, 0, 100)?;
            check_text("credit url", &credit.url, 0, 300)?;
        }
        if self.press.default.is_empty() && self.press.rows.iter().all(Vec::is_empty) {
            return Err("pack has no press sounds".into());
        }
        for set in [&self.press, &self.release] {
            if set.rows.len() > ROWS {
                return Err(format!("at most {ROWS} rows are supported"));
            }
        }
        let files = self.files();
        if files.len() > MAX_FILES {
            return Err(format!("pack references more than {MAX_FILES} files"));
        }
        if let Some(bad) = files.iter().find(|p| !valid_audio_path(p)) {
            return Err(format!("invalid sound path {bad:?}"));
        }
        Ok(())
    }

    /// Unique audio files the manifest references, in first-use order.
    pub fn files(&self) -> Vec<&str> {
        let mut seen = Vec::new();
        for path in self.press.paths().chain(self.release.paths()) {
            if !seen.contains(&path.as_str()) {
                seen.push(path.as_str());
            }
        }
        seen
    }
}

fn check_text(field: &str, value: &str, min: usize, max: usize) -> Result<(), String> {
    let len = value.chars().count();
    if len < min || len > max || value.chars().any(char::is_control) {
        return Err(format!(
            "{field} must be {min}-{max} characters without control characters"
        ));
    }
    Ok(())
}

/// Pack ids: lowercase letters, digits and single dashes, 1-48 characters.
pub fn valid_id(id: &str) -> bool {
    (1..=48).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
}

/// Relative sound paths: at most one folder level, simple names, audio extension.
/// Rejects anything that could escape the pack folder (`..`, absolute paths, `\`).
pub fn valid_audio_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 96 {
        return false;
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 2 {
        return false;
    }
    let simple = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    };
    if parts.len() == 2 && !simple(parts[0]) {
        return false;
    }
    match parts[parts.len() - 1].rsplit_once('.') {
        Some((stem, ext)) => {
            simple(stem) && AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
        }
        None => false,
    }
}

/// What the UI shows for a pack.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    pub source: Option<String>,
    pub builtin: bool,
}

impl PackInfo {
    fn new(m: &Manifest, builtin: bool) -> Self {
        Self {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
            author: m.author.clone(),
            license: m.license.clone(),
            kind: m.kind.clone(),
            description: m.description.clone(),
            source: m.source.clone(),
            builtin,
        }
    }
}

/// One decoded sound, mono, at its original sample rate.
pub struct Clip {
    pub samples: Box<[f32]>,
    pub rate: u32,
}

/// Clip indices for each key class. Empty lists fall back (see `pick`).
#[derive(Default)]
pub struct ClassMap {
    pub default: Vec<u16>,
    pub rows: [Vec<u16>; ROWS],
    pub space: Vec<u16>,
    pub enter: Vec<u16>,
    pub backspace: Vec<u16>,
}

impl ClassMap {
    /// Pick a clip for a key. Returns the clip and a playback-rate factor: when a
    /// pack has no dedicated space/enter/backspace sound, a generic one is played
    /// a little deeper so big keys still sound bigger.
    pub fn pick(&self, class: KeyClass, row: u8, roll: u32) -> Option<(u16, f32)> {
        let (special, fallback_rate): (&[u16], f32) = match class {
            KeyClass::Generic => (&[], 1.0),
            KeyClass::Space => (&self.space, 0.86),
            KeyClass::Enter => (&self.enter, 0.93),
            KeyClass::Backspace => (&self.backspace, 0.95),
        };
        let choose = |list: &[u16]| list[roll as usize % list.len()];
        if !special.is_empty() {
            return Some((choose(special), 1.0));
        }
        let row = (row as usize).min(ROWS - 1);
        if !self.rows[row].is_empty() {
            return Some((choose(&self.rows[row]), fallback_rate));
        }
        if !self.default.is_empty() {
            return Some((choose(&self.default), fallback_rate));
        }
        // Rows given but not this one: use the nearest row that has sounds.
        (1..ROWS)
            .flat_map(|d| [row.checked_sub(d), Some(row + d)])
            .flatten()
            .find(|&r| r < ROWS && !self.rows[r].is_empty())
            .map(|r| (choose(&self.rows[r]), fallback_rate))
    }
}

/// A fully decoded pack, ready for the audio thread.
pub struct SoundBank {
    pub id: String,
    pub clips: Vec<Clip>,
    pub press: ClassMap,
    pub release: ClassMap,
}

/// Where packs come from: the embedded set plus the user's packs folder.
pub struct PackStore {
    dir: PathBuf,
}

impl PackStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn list(&self) -> Vec<PackInfo> {
        let mut packs: Vec<PackInfo> = builtin_manifests()
            .iter()
            .map(|m| PackInfo::new(m, true))
            .collect();
        let mut installed: Vec<PackInfo> = self
            .installed_manifests()
            .iter()
            .map(|m| PackInfo::new(m, false))
            .collect();
        installed.sort_by_key(|p| p.name.to_lowercase());
        packs.extend(installed);
        packs
    }

    pub fn exists(&self, id: &str) -> bool {
        self.list().iter().any(|p| p.id == id)
    }

    pub fn is_builtin(id: &str) -> bool {
        builtin_manifests().iter().any(|m| m.id == id)
    }

    /// Manifests of installed packs. Invalid folders and ids that clash with a
    /// built-in pack are skipped.
    pub fn installed_manifests(&self) -> Vec<Manifest> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !valid_id(&name) || Self::is_builtin(&name) || !entry.path().is_dir() {
                continue;
            }
            match read_limited(&entry.path().join(MANIFEST), MAX_MANIFEST_BYTES)
                .and_then(|bytes| Manifest::parse(&bytes))
            {
                Ok(m) if m.id == name => out.push(m),
                Ok(m) => eprintln!("pack folder {name:?} holds pack id {:?}, skipped", m.id),
                Err(e) => eprintln!("pack {name:?} skipped: {e}"),
            }
        }
        out
    }

    pub fn load_bank(&self, id: &str) -> Result<SoundBank, String> {
        if let Some(manifest) = builtin_manifests().into_iter().find(|m| m.id == id) {
            let prefix = format!("{id}/");
            return build_bank(&manifest, |rel| {
                BUILTIN_FILES
                    .iter()
                    .find(|(path, _)| path.strip_prefix(&prefix) == Some(rel))
                    .map(|(_, bytes)| bytes.to_vec())
                    .ok_or_else(|| format!("missing built-in file {rel}"))
            });
        }
        if !valid_id(id) {
            return Err(format!("invalid pack id {id:?}"));
        }
        load_dir_bank(&self.dir.join(id), Some(id))
    }
}

/// Load a pack straight from a folder. Used for installed packs and to verify a
/// marketplace download before it is moved into place.
pub fn load_dir_bank(dir: &Path, expected_id: Option<&str>) -> Result<SoundBank, String> {
    let manifest = Manifest::parse(&read_limited(&dir.join(MANIFEST), MAX_MANIFEST_BYTES)?)?;
    if let Some(expected) = expected_id {
        if manifest.id != expected {
            return Err(format!(
                "pack.json id {:?} does not match {expected:?}",
                manifest.id
            ));
        }
    }
    let mut total = 0u64;
    build_bank(&manifest, |rel| {
        let bytes = read_limited(&dir.join(rel), MAX_FILE_BYTES)?;
        total += bytes.len() as u64;
        if total > MAX_PACK_BYTES {
            return Err("pack is larger than 16 MB".into());
        }
        Ok(bytes)
    })
}

fn read_limited(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.len() > max {
        return Err(format!("{} is missing or too large", path.display()));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn builtin_manifests() -> Vec<Manifest> {
    BUILTIN_FILES
        .iter()
        .filter(|(path, _)| path.ends_with("/pack.json"))
        .filter_map(|(path, bytes)| match Manifest::parse(bytes) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("built-in pack {path} is invalid: {e}");
                None
            }
        })
        .collect()
}

fn build_bank(
    manifest: &Manifest,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<SoundBank, String> {
    let mut clips = Vec::new();
    let mut index: HashMap<&str, u16> = HashMap::new();
    for rel in manifest.files() {
        let ext = rel.rsplit('.').next().unwrap_or_default();
        let clip = decode(read(rel)?, ext).map_err(|e| format!("{rel}: {e}"))?;
        index.insert(rel, clips.len() as u16);
        clips.push(clip);
    }
    let map = |list: &[String]| list.iter().map(|p| index[p.as_str()]).collect::<Vec<u16>>();
    let class_map = |set: &SoundSet| {
        let mut rows: [Vec<u16>; ROWS] = Default::default();
        for (slot, list) in rows.iter_mut().zip(&set.rows) {
            *slot = map(list);
        }
        ClassMap {
            default: map(&set.default),
            rows,
            space: map(&set.space),
            enter: map(&set.enter),
            backspace: map(&set.backspace),
        }
    };
    Ok(SoundBank {
        id: manifest.id.clone(),
        press: class_map(&manifest.press),
        release: class_map(&manifest.release),
        clips,
    })
}

/// Decode an audio file to mono f32 and trim the silence around the hit so the
/// click starts on the first sample.
pub fn decode(bytes: Vec<u8>, extension: &str) -> Result<Clip, String> {
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| format!("unsupported audio: {e}"))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("no audio track")?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("no audio parameters")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| format!("unsupported codec: {e}"))?;

    let mut mono: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    let mut interleaved: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(DecodeError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("read error: {e}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(DecodeError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode error: {e}")),
        };
        let spec = buffer.spec();
        let channels = spec.channels().count().max(1);
        rate = spec.rate();
        if !(8_000..=192_000).contains(&rate) {
            return Err(format!("unsupported sample rate {rate}"));
        }
        buffer.copy_to_vec_interleaved(&mut interleaved);
        mono.extend(
            interleaved
                .chunks_exact(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
        if mono.len() > MAX_CLIP_SECONDS * rate as usize {
            return Err(format!("sound is longer than {MAX_CLIP_SECONDS} s"));
        }
    }
    trim(&mut mono, rate)?;
    Ok(Clip {
        samples: mono.into_boxed_slice(),
        rate,
    })
}

/// Cut leading and trailing silence, keep 1 ms before the attack, fade the tail.
fn trim(samples: &mut Vec<f32>, rate: u32) -> Result<(), String> {
    const START_THRESHOLD: f32 = 0.004;
    const END_THRESHOLD: f32 = 0.0015;
    let ms = (rate / 1000).max(1) as usize;
    let first = samples
        .iter()
        .position(|s| s.is_finite() && s.abs() >= START_THRESHOLD)
        .ok_or("sound is silent")?;
    let last = samples
        .iter()
        .rposition(|s| s.is_finite() && s.abs() >= END_THRESHOLD)
        .unwrap_or(first);
    let start = first.saturating_sub(ms);
    let end = (last + 10 * ms).min(samples.len());
    samples.truncate(end);
    samples.drain(..start);
    for s in samples.iter_mut() {
        if !s.is_finite() {
            *s = 0.0;
        }
    }
    let fade = (3 * ms).min(samples.len());
    let len = samples.len();
    for (i, s) in samples[len - fade..].iter_mut().enumerate() {
        *s *= 1.0 - (i + 1) as f32 / fade as f32;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_pack_parses_and_decodes() {
        let manifests = builtin_manifests();
        assert_eq!(manifests.len(), 13);
        assert!(manifests.iter().any(|m| m.id == DEFAULT_PACK));
        let store = PackStore::new(std::env::temp_dir().join("tubbykeys-test-none"));
        for m in manifests {
            let bank = store
                .load_bank(&m.id)
                .unwrap_or_else(|e| panic!("{}: {e}", m.id));
            assert!(!bank.clips.is_empty());
            for clip in &bank.clips {
                assert!(clip.samples.len() > 100, "{} has a near-empty clip", m.id);
                assert!(clip.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.5));
            }
            assert!(bank.press.pick(KeyClass::Generic, 2, 0).is_some());
            assert!(bank.press.pick(KeyClass::Space, 4, 0).is_some());
        }
    }

    #[test]
    fn decoded_clips_start_on_the_attack() {
        let store = PackStore::new(std::env::temp_dir());
        let bank = store.load_bank("holy-panda").unwrap();
        for clip in &bank.clips {
            let ms = (clip.rate / 1000) as usize;
            let head = &clip.samples[..(2 * ms).min(clip.samples.len())];
            assert!(
                head.iter().any(|s| s.abs() >= 0.004),
                "attack is later than 2 ms"
            );
        }
    }

    #[test]
    fn paths_that_escape_the_pack_are_rejected() {
        for bad in [
            "../x.wav",
            "a/../x.wav",
            "/x.wav",
            "C:/x.wav",
            "a\\x.wav",
            "a/b/c.wav",
            "x.exe",
            "x",
            ".wav",
            "a/.wav",
            "x.wav.exe",
            "",
            "a b.wav",
        ] {
            assert!(!valid_audio_path(bad), "{bad} should be rejected");
        }
        for good in [
            "x.wav",
            "press/GENERIC_R0.mp3",
            "a-b_c.OGG",
            "release/x.flac",
        ] {
            assert!(valid_audio_path(good), "{good} should be accepted");
        }
    }

    #[test]
    fn ids_are_strict() {
        assert!(valid_id("holy-panda"));
        assert!(valid_id("pack2"));
        for bad in [
            "",
            "-a",
            "a-",
            "a--b",
            "A",
            "a_b",
            "a.b",
            "../a",
            &"a".repeat(49),
        ] {
            assert!(!valid_id(bad), "{bad} should be rejected");
        }
    }

    #[test]
    fn manifest_validation_catches_bad_packs() {
        let base = r#"{"schema":1,"id":"x","name":"X","press":{"default":["a.wav"]}}"#;
        assert!(Manifest::parse(base.as_bytes()).is_ok());
        let cases = [
            r#"{"schema":2,"id":"x","name":"X","press":{"default":["a.wav"]}}"#,
            r#"{"schema":1,"id":"X","name":"X","press":{"default":["a.wav"]}}"#,
            r#"{"schema":1,"id":"x","name":"","press":{"default":["a.wav"]}}"#,
            r#"{"schema":1,"id":"x","name":"X","press":{}}"#,
            r#"{"schema":1,"id":"x","name":"X","press":{"default":["../a.wav"]}}"#,
            r#"{"schema":1,"id":"x","name":"X","press":{"rows":[[],[],[],[],[],["a.wav"]]}}"#,
        ];
        for case in cases {
            assert!(
                Manifest::parse(case.as_bytes()).is_err(),
                "{case} should fail"
            );
        }
    }

    #[test]
    fn pick_falls_back_sensibly() {
        let map = ClassMap {
            rows: [vec![0], vec![1], vec![], vec![3], vec![4]],
            ..Default::default()
        };
        assert_eq!(map.pick(KeyClass::Generic, 1, 0), Some((1, 1.0)));
        // Missing row 2 uses a neighbour.
        assert!(matches!(
            map.pick(KeyClass::Generic, 2, 0),
            Some((1 | 3, _))
        ));
        // No dedicated space sound: row 4, pitched down.
        assert_eq!(map.pick(KeyClass::Space, 4, 0), Some((4, 0.86)));
        let empty = ClassMap::default();
        assert_eq!(empty.pick(KeyClass::Generic, 0, 0), None);
    }
}
