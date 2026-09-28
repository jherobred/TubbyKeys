//! Community marketplace. The catalogue is a static `index.json` in the
//! TubbyKeys GitHub repository. Packs download file by file over HTTPS from
//! that one host, must match the size and SHA-256 listed in the index, must
//! decode cleanly, and only then move into the packs folder. No accounts, no
//! tracking: the only traffic is the file requests themselves.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::packs::{
    self, valid_audio_path, valid_id, PackStore, MANIFEST, MAX_FILES, MAX_FILE_BYTES,
    MAX_PACK_BYTES,
};

pub const REGISTRY: &str = "https://raw.githubusercontent.com/jherobred/TubbyKeys/main/marketplace";
const INDEX_MAX_BYTES: u64 = 1024 * 1024;
const MAX_PACKS: usize = 2000;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Index {
    pub schema: u32,
    pub packs: Vec<Entry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub description: String,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

impl Entry {
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.id) {
            return Err(format!("invalid pack id {:?}", self.id));
        }
        let text_ok =
            |s: &str, max: usize| s.chars().count() <= max && !s.chars().any(char::is_control);
        if self.name.is_empty()
            || !text_ok(&self.name, 64)
            || !text_ok(&self.version, 32)
            || !text_ok(&self.author, 200)
            || !text_ok(&self.license, 64)
            || !text_ok(&self.kind, 16)
            || !text_ok(&self.description, 300)
        {
            return Err(format!("{}: invalid text fields", self.id));
        }
        if self.files.is_empty() || self.files.len() > MAX_FILES + 1 {
            return Err(format!("{}: bad file count", self.id));
        }
        let mut seen = HashSet::new();
        let mut total = 0u64;
        for file in &self.files {
            if file.path != MANIFEST && !valid_audio_path(&file.path) {
                return Err(format!("{}: invalid file path {:?}", self.id, file.path));
            }
            // Windows paths are case-insensitive.
            if !seen.insert(file.path.to_ascii_lowercase()) {
                return Err(format!("{}: duplicate file {:?}", self.id, file.path));
            }
            if file.size == 0 || file.size > MAX_FILE_BYTES {
                return Err(format!("{}: {} has a bad size", self.id, file.path));
            }
            let hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
            if file.sha256.len() != 64 || !file.sha256.bytes().all(hex) {
                return Err(format!("{}: {} has a bad checksum", self.id, file.path));
            }
            total += file.size;
        }
        if !seen.contains(MANIFEST) {
            return Err(format!("{}: pack.json missing", self.id));
        }
        if total > MAX_PACK_BYTES {
            return Err(format!("{}: pack is larger than 16 MB", self.id));
        }
        Ok(())
    }
}

/// Parse the catalogue. Invalid entries, duplicates and ids that clash with a
/// built-in pack are dropped rather than failing the whole list.
pub fn parse_index(bytes: &[u8]) -> Result<Index, String> {
    let mut index: Index =
        serde_json::from_slice(bytes).map_err(|e| format!("marketplace index is invalid: {e}"))?;
    if index.schema != 1 {
        return Err(format!(
            "marketplace index schema {} needs a newer TubbyKeys",
            index.schema
        ));
    }
    if index.packs.len() > MAX_PACKS {
        return Err("marketplace index is too large".into());
    }
    let mut ids = HashSet::new();
    index.packs.retain(|entry| match entry.validate() {
        Ok(()) => !PackStore::is_builtin(&entry.id) && ids.insert(entry.id.clone()),
        Err(e) => {
            eprintln!("marketplace entry skipped: {e}");
            false
        }
    });
    Ok(index)
}

pub trait Fetch {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String>;
}

/// HTTPS only, no redirects, bounded body sizes, 30 s timeout.
pub struct Https(ureq::Agent);

impl Https {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("TubbyKeys/", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        Self(agent)
    }
}

impl Fetch for Https {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut response = self
            .0
            .get(url)
            .call()
            .map_err(|e| format!("download failed: {e}"))?;
        response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(|e| format!("download failed: {e}"))
    }
}

pub fn fetch_index(fetch: &dyn Fetch) -> Result<Index, String> {
    parse_index(&fetch.get(&format!("{REGISTRY}/index.json"), INDEX_MAX_BYTES)?)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Download, verify and install a pack. Replaces an older installed version.
pub fn install(fetch: &dyn Fetch, entry: &Entry, packs_dir: &Path) -> Result<(), String> {
    entry.validate()?;
    if PackStore::is_builtin(&entry.id) {
        return Err(format!("{} is a built-in pack", entry.id));
    }
    let io = |e: std::io::Error| format!("could not write the pack: {e}");
    std::fs::create_dir_all(packs_dir).map_err(io)?;
    // Folder names starting with '.' are never loaded as packs.
    let staging = packs_dir.join(format!(".download-{}", entry.id));
    let _ = std::fs::remove_dir_all(&staging);
    let result = (|| {
        for file in &entry.files {
            let url = format!("{REGISTRY}/packs/{}/{}", entry.id, file.path);
            let bytes = fetch.get(&url, file.size)?;
            if bytes.len() as u64 != file.size {
                return Err(format!("{}: size does not match the index", file.path));
            }
            if sha256_hex(&bytes) != file.sha256 {
                return Err(format!("{}: checksum does not match the index", file.path));
            }
            let target = staging.join(&file.path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            std::fs::write(&target, &bytes).map_err(io)?;
        }
        // Every sound the manifest names must be among the verified files and decode.
        packs::load_dir_bank(&staging, Some(&entry.id))?;
        let destination = packs_dir.join(&entry.id);
        if destination.exists() {
            std::fs::remove_dir_all(&destination).map_err(io)?;
        }
        std::fs::rename(&staging, &destination).map_err(io)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

pub fn remove(packs_dir: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) || PackStore::is_builtin(id) {
        return Err(format!("{id} cannot be removed"));
    }
    std::fs::remove_dir_all(packs_dir.join(id)).map_err(|e| format!("could not remove {id}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct Fake(HashMap<String, Vec<u8>>);

    impl Fetch for Fake {
        fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
            let bytes = self.0.get(url).cloned().ok_or("404")?;
            if bytes.len() as u64 > limit {
                return Err("body over limit".into());
            }
            Ok(bytes)
        }
    }

    fn builtin(path: &str) -> Vec<u8> {
        packs::BUILTIN_FILES
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, b)| b.to_vec())
            .unwrap()
    }

    /// A one-file community pack built from a real recording.
    fn fixture(id: &str) -> (Fake, Entry) {
        let manifest = format!(
            r#"{{"schema":1,"id":"{id}","name":"Test","license":"CC0-1.0","press":{{"default":["a.mp3"]}}}}"#
        );
        let files = [
            ("pack.json", manifest.into_bytes()),
            ("a.mp3", builtin("cream/press/GENERIC_R2.mp3")),
        ];
        let mut served = HashMap::new();
        let mut entries = Vec::new();
        for (path, bytes) in files {
            entries.push(FileEntry {
                path: path.into(),
                size: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            });
            served.insert(format!("{REGISTRY}/packs/{id}/{path}"), bytes);
        }
        let entry = Entry {
            id: id.into(),
            name: "Test".into(),
            version: "1.0.0".into(),
            author: "a".into(),
            license: "CC0-1.0".into(),
            kind: "linear".into(),
            description: String::new(),
            files: entries,
        };
        (Fake(served), entry)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tubbykeys-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn installs_a_verified_pack() {
        let dir = temp_dir("install");
        let (fetch, entry) = fixture("test-pack");
        install(&fetch, &entry, &dir).unwrap();
        let store = PackStore::new(dir.clone());
        let installed: Vec<String> = store
            .installed_manifests()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(installed, ["test-pack"]);
        assert!(store.load_bank("test-pack").is_ok());
        remove(&dir, "test-pack").unwrap();
        assert!(!dir.join("test-pack").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tampered_files_are_rejected_and_cleaned_up() {
        let dir = temp_dir("tampered");
        let (mut fetch, entry) = fixture("test-pack");
        let url = format!("{REGISTRY}/packs/test-pack/a.mp3");
        fetch.0.get_mut(&url).unwrap()[100] ^= 0xFF;
        assert!(install(&fetch, &entry, &dir)
            .unwrap_err()
            .contains("checksum"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn manifest_cannot_reference_unlisted_files() {
        let dir = temp_dir("unlisted");
        let (mut fetch, mut entry) = fixture("test-pack");
        let manifest =
            br#"{"schema":1,"id":"test-pack","name":"T","press":{"default":["a.mp3","b.mp3"]}}"#
                .to_vec();
        entry.files[0].size = manifest.len() as u64;
        entry.files[0].sha256 = sha256_hex(&manifest);
        fetch
            .0
            .insert(format!("{REGISTRY}/packs/test-pack/pack.json"), manifest);
        assert!(install(&fetch, &entry, &dir).is_err());
        assert!(!dir.join("test-pack").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn index_drops_unsafe_or_conflicting_entries() {
        let (_, good) = fixture("good-pack");
        let mut traversal = good.clone();
        traversal.id = "evil".into();
        traversal.files[1].path = "../../evil.mp3".into();
        let mut builtin_clash = good.clone();
        builtin_clash.id = "holy-panda".into();
        let mut oversized = good.clone();
        oversized.id = "big".into();
        oversized.files[1].size = MAX_FILE_BYTES + 1;
        let index = Index {
            schema: 1,
            packs: vec![good.clone(), traversal, builtin_clash, oversized, good],
        };
        let parsed = parse_index(&serde_json::to_vec(&index).unwrap()).unwrap();
        let ids: Vec<&str> = parsed.packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["good-pack"]);
    }

    #[test]
    fn builtin_packs_cannot_be_removed() {
        assert!(remove(&std::env::temp_dir(), "holy-panda").is_err());
        assert!(remove(&std::env::temp_dir(), "../x").is_err());
    }
}
