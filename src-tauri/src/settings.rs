//! User settings, stored as JSON in the roaming app config folder.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::packs::DEFAULT_PACK;

pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+K";

/// How the visualizer looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// Mini keyboard with a combo counter.
    Keyboard,
    /// Small capsule with just the combo.
    Pill,
    /// Thin fluid line along a screen edge that ripples where you type.
    Wave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    Top,
    Bottom,
    Random,
    /// Wherever the user dragged it (see `Visualizer::position`).
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Small,
    Medium,
    Large,
}

impl Size {
    pub fn scale(self) -> f64 {
        match self {
            Size::Small => 0.8,
            Size::Medium => 1.0,
            Size::Large => 1.25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Animation {
    Pop,
    Slide,
    Bounce,
    Pulse,
}

/// What the overlay does once you stop typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Idle {
    Hide,
    Fade,
    Keep,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Visualizer {
    pub enabled: bool,
    pub style: Style,
    pub size: Size,
    pub show_combo: bool,
    pub placement: Placement,
    /// Where the user dropped it: the window's top-left in screen pixels.
    pub position: Option<[f64; 2]>,
    pub animation: Animation,
    /// Milliseconds without a key before the combo resets. 0 keeps it forever.
    pub combo_timeout_ms: u32,
    pub idle: Idle,
    /// Get out of the way of full-screen games, videos and presentations.
    pub hide_in_fullscreen: bool,
    /// Keep the overlay out of screenshots, recordings and screen shares.
    pub hide_from_capture: bool,
}

impl Default for Visualizer {
    fn default() -> Self {
        Self {
            enabled: false,
            style: Style::Keyboard,
            size: Size::Medium,
            show_combo: true,
            placement: Placement::Bottom,
            position: None,
            animation: Animation::Pop,
            combo_timeout_ms: 2000,
            idle: Idle::Fade,
            hide_in_fullscreen: true,
            hide_from_capture: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    pub pack_id: String,
    pub volume: f32,
    /// -1 thock .. 1 clack.
    pub tone: f32,
    /// -1 deep .. 1 sharp.
    pub pitch: f32,
    pub randomize_pitch: bool,
    pub spatial: bool,
    pub stereo_width: f32,
    /// -1 left only .. 0 middle .. 1 right only.
    pub balance: f32,
    /// Narrow the stereo field when headphones are the output.
    pub headphone_width: bool,
    pub hotkey: String,
    pub visualizer: Visualizer,
    /// The tray icon squishes with every key press.
    pub tray_pulse: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            pack_id: DEFAULT_PACK.into(),
            volume: 0.7,
            tone: 0.0,
            pitch: 0.0,
            randomize_pitch: true,
            spatial: true,
            stereo_width: 0.8,
            balance: 0.0,
            headphone_width: true,
            hotkey: DEFAULT_HOTKEY.into(),
            visualizer: Visualizer::default(),
            tray_pulse: true,
        }
    }
}

impl Settings {
    /// Clamp everything into range so a hand-edited file cannot break the engine.
    pub fn sanitized(mut self) -> Self {
        let unit = |v: f32, lo: f32, fallback: f32| {
            if v.is_finite() {
                v.clamp(lo, 1.0)
            } else {
                fallback
            }
        };
        self.volume = unit(self.volume, 0.0, 0.7);
        self.tone = unit(self.tone, -1.0, 0.0);
        self.pitch = unit(self.pitch, -1.0, 0.0);
        self.stereo_width = unit(self.stereo_width, 0.0, 0.8);
        self.balance = unit(self.balance, -1.0, 0.0);
        self.hotkey = self.hotkey.chars().take(64).collect();
        self.pack_id = self.pack_id.chars().take(48).collect();
        let v = &mut self.visualizer;
        v.combo_timeout_ms = v.combo_timeout_ms.min(60_000);
        v.position = v
            .position
            .filter(|p| p.iter().all(|c| c.is_finite() && c.abs() < 1e6));
        if v.placement == Placement::Custom && v.position.is_none() {
            v.placement = Placement::Bottom;
        }
        // The wave hugs a screen edge; it cannot float.
        if v.style == Style::Wave && !matches!(v.placement, Placement::Top | Placement::Bottom) {
            v.placement = Placement::Bottom;
        }
        self
    }

    /// Stereo width the engine should use for the current output.
    pub fn effective_width(&self, headphones: bool) -> f32 {
        if headphones && self.headphone_width {
            self.stereo_width * 0.55
        } else {
            self.stereo_width
        }
    }
}

/// Load settings. Returns defaults and `true` on first run or if the file is unreadable.
pub fn load(path: &Path) -> (Settings, bool) {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
            Ok(settings) => (settings.sanitized(), false),
            Err(e) => {
                eprintln!("settings file is invalid, using defaults: {e}");
                (Settings::default(), false)
            }
        },
        Err(_) => (Settings::default(), true),
    }
}

/// Write atomically: a crash mid-write never leaves a half-written file.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_files_fill_in_defaults() {
        let s: Settings =
            serde_json::from_str(r#"{"volume":0.2,"visualizer":{"enabled":true}}"#).unwrap();
        assert_eq!(s.volume, 0.2);
        assert!(s.visualizer.enabled);
        assert_eq!(s.visualizer.combo_timeout_ms, 2000);
        assert_eq!(s.pack_id, DEFAULT_PACK);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let s = Settings {
            volume: 5.0,
            tone: f32::NAN,
            pitch: -3.0,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!((s.volume, s.tone, s.pitch), (1.0, 0.0, -1.0));
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("tubbykeys-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let settings = Settings {
            pack_id: "topre".into(),
            ..Settings::default()
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path), (settings, false));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn placements_stay_consistent() {
        let mut s = Settings::default();
        s.visualizer.placement = Placement::Custom;
        assert_eq!(
            s.clone().sanitized().visualizer.placement,
            Placement::Bottom
        );
        s.visualizer.position = Some([2.0, f64::NAN]);
        assert_eq!(s.clone().sanitized().visualizer.position, None);
        s.visualizer.position = Some([1920.0, -40.0]);
        assert_eq!(
            s.clone().sanitized().visualizer.position,
            Some([1920.0, -40.0])
        );
        s.visualizer.style = Style::Wave;
        assert_eq!(s.sanitized().visualizer.placement, Placement::Bottom);
    }

    #[test]
    fn headphones_narrow_the_stereo_field() {
        let s = Settings::default();
        assert!(s.effective_width(true) < s.effective_width(false));
    }
}
