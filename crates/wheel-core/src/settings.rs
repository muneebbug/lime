use serde::{Deserialize, Serialize};

/// Settings schema.
///
/// Lime has not shipped to users, so there is no migration story: the schema is
/// simply edited in place, and a settings file that no longer parses falls back
/// to defaults. Add a migration only once there are real installs to preserve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WheelSettings {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub trigger: TriggerSettings,
    #[serde(default)]
    pub wheel_ui: WheelUiSettings,
    #[serde(default)]
    pub output: OutputSettings,
    #[serde(default)]
    pub ffmpeg: FfmpegSettings,
    #[serde(default = "default_presets")]
    pub presets: Vec<ActionPreset>,
}

/// Current schema version, written so a future change can spot old files.
pub const SETTINGS_VERSION: u32 = 2;

fn default_version() -> u32 {
    SETTINGS_VERSION
}

impl Default for WheelSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            general: Default::default(),
            trigger: Default::default(),
            wheel_ui: Default::default(),
            output: Default::default(),
            ffmpeg: Default::default(),
            presets: default_presets(),
        }
    }
}

impl WheelSettings {
    /// Load settings from a JSON file, or return default settings if the file does not exist.
    pub fn load_or_default<P: AsRef<std::path::Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(mut settings) = serde_json::from_str::<WheelSettings>(&content) {
                    // Stamp the current version so a file written by an older
                    // build is recognisable later.
                    settings.version = SETTINGS_VERSION;
                    if settings.presets.is_empty() {
                        settings.presets = default_presets();
                    }
                    return settings;
                }
            }
        }
        Self::default()
    }

    /// Load settings and report whether this is a first run for onboarding.
    ///
    /// Onboarding is a first-run experience, so it appears when there is no
    /// usable history: no settings file, or one we could not parse (in which case
    /// the settings themselves have already fallen back to defaults).
    pub fn load_reporting_first_run<P: AsRef<std::path::Path>>(path: P) -> (Self, bool) {
        let path = path.as_ref();
        let is_first_run = !path.exists()
            || std::fs::read_to_string(path)
                .ok()
                .and_then(|content| serde_json::from_str::<WheelSettings>(&content).ok())
                .is_none();

        (Self::load_or_default(path), is_first_run)
    }

    /// Save settings to a JSON file, creating parent directories if necessary.
    pub fn save_to_path<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneralSettings {
    pub launch_at_login: bool,
    pub language: String,
    pub auto_update: bool,
    pub minimize_to_tray: bool,
    #[serde(default = "default_true")]
    pub include_prereleases: bool,
    /// Whether the first-run onboarding window has been finished. Once true the
    /// window is never shown again, even if FFmpeg was skipped.
    #[serde(default)]
    pub onboarding_completed: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            language: "en".into(),
            auto_update: true,
            minimize_to_tray: true,
            include_prereleases: true,
            onboarding_completed: false,
        }
    }
}

/// Where Lime looks for `ffmpeg.exe`, and what it downloaded itself.
///
/// FFmpeg is not bundled. Image conversions do not need it; video, audio and
/// PDF output do.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FfmpegSettings {
    /// An explicit `ffmpeg.exe` chosen by the user. Wins over every other
    /// location, and is skipped silently if it later stops working.
    #[serde(default)]
    pub custom_path: Option<String>,
    /// Version line of the copy Lime downloaded, used to notice a newer one.
    #[serde(default)]
    pub managed_version: Option<String>,
    /// Set once the user has declined the download, so onboarding can explain
    /// the consequence without nagging on every launch.
    #[serde(default)]
    pub declined_download: bool,
}

impl Default for FfmpegSettings {
    fn default() -> Self {
        Self {
            custom_path: None,
            managed_version: None,
            declined_download: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerModifier {
    Shift,
    Ctrl,
    Alt,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TriggerSettings {
    /// Which modifier key must be held while dragging to arm the trigger
    pub modifier: TriggerModifier,
    /// Minimum mouse movement in pixels before arming (DPI-independent at 96 DPI base)
    pub movement_threshold_px: u32,
    /// How long to wait for OLE DragEnter before cancelling (milliseconds)
    pub confirm_timeout_ms: u32,
    /// Show the wheel even for file drags without a modifier
    pub always_show: bool,
    /// Pause: when true, the wheel is globally disabled
    pub paused: bool,
}

impl Default for TriggerSettings {
    fn default() -> Self {
        Self {
            modifier: TriggerModifier::Shift,
            movement_threshold_px: 6,
            confirm_timeout_ms: 250,
            always_show: false,
            paused: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WheelUiSettings {
    pub slot_count: u32,
    pub theme: Theme,
    pub animation_speed: f32,
    pub reduced_motion: bool,
    pub size: WheelSize,
    pub corner_radius: u32,
    pub sound_enabled: bool,
    pub context_filter_enabled: bool,
    /// Vertical anchor of the detached status/progress HUD.
    #[serde(default)]
    pub status_vertical: StatusVertical,
    /// Horizontal anchor of the detached status/progress HUD.
    #[serde(default)]
    pub status_horizontal: StatusHorizontal,
    /// Layout density of the detached status/progress HUD.
    #[serde(default)]
    pub hud_style: HudStyle,
}

/// How much detail the status/progress HUD shows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HudStyle {
    /// Icon above the message, with the file names underneath.
    Standard,
    /// Single line: icon beside the message, no file names.
    Compact,
}

impl Default for HudStyle {
    fn default() -> Self {
        HudStyle::Standard
    }
}

impl Default for WheelUiSettings {
    fn default() -> Self {
        Self {
            slot_count: 8,
            theme: Theme::System,
            animation_speed: 1.0,
            reduced_motion: false,
            size: WheelSize::Medium,
            corner_radius: 16,
            sound_enabled: true,
            context_filter_enabled: true,
            status_vertical: StatusVertical::default(),
            status_horizontal: StatusHorizontal::default(),
            hud_style: HudStyle::default(),
        }
    }
}

/// Vertical anchor for the status/progress HUD, as a fraction of the work area.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StatusVertical {
    Top,
    Middle,
    Bottom,
}

impl Default for StatusVertical {
    fn default() -> Self {
        StatusVertical::Bottom
    }
}

impl StatusVertical {
    /// 0.0 = work area top, 0.5 = centred, 1.0 = work area bottom.
    pub fn anchor(self) -> f32 {
        match self {
            StatusVertical::Top => 0.0,
            StatusVertical::Middle => 0.5,
            StatusVertical::Bottom => 1.0,
        }
    }
}

/// Horizontal anchor for the status/progress HUD, as a fraction of the work area.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StatusHorizontal {
    Left,
    Center,
    Right,
}

impl Default for StatusHorizontal {
    fn default() -> Self {
        StatusHorizontal::Center
    }
}

impl StatusHorizontal {
    /// 0.0 = work area left, 0.5 = centred, 1.0 = work area right.
    pub fn anchor(self) -> f32 {
        match self {
            StatusHorizontal::Left => 0.0,
            StatusHorizontal::Center => 0.5,
            StatusHorizontal::Right => 1.0,
        }
    }
}

/// Radial wheel diameter preset. Stored as a name so the pixel geometry behind
/// each option stays an implementation detail.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WheelSize {
    Small,
    Medium,
    Large,
}

impl WheelSize {
    /// Outer diameter in CSS pixels. Must stay within the fixed 400x400 overlay window.
    pub fn px(self) -> u32 {
        match self {
            WheelSize::Small => 280,
            WheelSize::Medium => 320,
            WheelSize::Large => 400,
        }
    }
}

impl Default for WheelSize {
    fn default() -> Self {
        WheelSize::Medium
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputPolicy {
    /// Save next to the source file (default)
    #[default]
    NextToSource,
    /// Save to a fixed folder
    FixedFolder,
    /// Copy result to clipboard
    Clipboard,
}

/// Reads a policy, tolerating casing and separator differences.
///
/// Lime is unreleased, so nothing older needs migrating; this exists only so a
/// hand-edited settings file cannot fail to parse and reset every preference.
impl<'de> Deserialize<'de> for OutputPolicy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        let key: String = name
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();

        Ok(match key.as_str() {
            "fixedfolder" => OutputPolicy::FixedFolder,
            "clipboard" => OutputPolicy::Clipboard,
            _ => OutputPolicy::NextToSource,
        })
    }
}

/// Per-media-type metadata preservation.
///
/// EXIF, ICC profiles, chapters and cover art all cost bytes, so they are
/// opt-out per media type rather than one global switch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataSettings {
    /// Keep EXIF/XMP/IPTC/ICC when converting images.
    pub images: bool,
    /// Keep tags, chapters and cover art when converting audio.
    pub audio: bool,
    /// Keep tags and chapters when converting video.
    pub video: bool,
}

impl Default for MetadataSettings {
    fn default() -> Self {
        Self {
            images: true,
            audio: true,
            video: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutputSettings {
    #[serde(default)]
    pub policy: OutputPolicy,
    #[serde(default)]
    pub fixed_folder: Option<String>,
    /// Suffix added between stem and extension: e.g. ".converted" → file.converted.png
    #[serde(default)]
    pub suffix: String,
    /// Whether to overwrite the source file (default: false, very dangerous)
    #[serde(default)]
    pub overwrite_source: bool,
    /// Send source to Recycle Bin after a successful conversion
    #[serde(default)]
    pub recycle_source: bool,
    #[serde(default)]
    pub metadata: MetadataSettings,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            policy: OutputPolicy::NextToSource,
            fixed_folder: None,
            suffix: String::new(),
            overwrite_source: false,
            recycle_source: false,
            metadata: MetadataSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresetStep {
    pub action_id: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub steps: Vec<PresetStep>,
    pub enabled: bool,
}

pub fn default_presets() -> Vec<ActionPreset> {
    vec![
        ActionPreset {
            id: "preset.trim_webp".into(),
            name: "Trim & WebP".into(),
            description: "Trim transparent blank pixels and convert to WebP".into(),
            icon: "sparkles".into(),
            steps: vec![
                PresetStep { action_id: "tool.trim".into(), params: serde_json::json!({}) },
                PresetStep { action_id: "convert.webp".into(), params: serde_json::json!({}) },
            ],
            enabled: true,
        },
        ActionPreset {
            id: "preset.trim_png".into(),
            name: "Trim PNG".into(),
            description: "Trim transparent blank pixels from PNG".into(),
            icon: "scissors".into(),
            steps: vec![
                PresetStep { action_id: "tool.trim".into(), params: serde_json::json!({}) },
                PresetStep { action_id: "convert.png".into(), params: serde_json::json!({}) },
            ],
            enabled: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_and_onboarding_fields_round_trip() {
        // Fresh installs must see onboarding, and whatever the user does on that
        // screen has to survive a save/load cycle or it is lost on next launch.
        let fresh = WheelSettings::default();
        assert!(!fresh.general.onboarding_completed);
        assert!(fresh.ffmpeg.custom_path.is_none());
        assert!(!fresh.ffmpeg.declined_download);

        let mut done = fresh.clone();
        done.general.onboarding_completed = true;
        done.ffmpeg.custom_path = Some("C:/tools/ffmpeg.exe".into());
        done.ffmpeg.managed_version = Some("7.1".into());

        let dir = std::env::temp_dir().join("wheel_settings_roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");

        done.save_to_path(&path).unwrap();
        let loaded = WheelSettings::load_or_default(&path);

        assert!(loaded.general.onboarding_completed);
        assert_eq!(loaded.ffmpeg.custom_path.as_deref(), Some("C:/tools/ffmpeg.exe"));
        assert_eq!(loaded.ffmpeg.managed_version.as_deref(), Some("7.1"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_settings_without_the_new_fields_still_load() {
        // A file with no `onboarding_completed` or `ffmpeg` key must still
        // deserialize, so an absent key means "not done" rather than a parse error.
        let json = r#"{
            "version": 2,
            "general": {
                "launch_at_login": false,
                "language": "en",
                "auto_update": true,
                "minimize_to_tray": true,
                "include_prereleases": true
            }
        }"#;

        let parsed: WheelSettings = serde_json::from_str(json).expect("file must parse");
        assert!(!parsed.general.onboarding_completed);
        assert!(parsed.ffmpeg.custom_path.is_none());
        assert_eq!(parsed.ffmpeg, FfmpegSettings::default());
    }

    #[test]
    fn test_wheel_size_serialises_as_snake_case() {
        // The wire format is snake_case, and the frontend size table is keyed on
        // it. Pinning the exact strings catches a rename that would otherwise
        // only show up as the wheel silently refusing to change size.
        for (size, expected) in [
            (WheelSize::Small, "\"small\""),
            (WheelSize::Medium, "\"medium\""),
            (WheelSize::Large, "\"large\""),
        ] {
            assert_eq!(serde_json::to_string(&size).unwrap(), expected);
        }
    }

    #[test]
    fn test_wheel_size_rejects_values_it_does_not_know() {
        // No compatibility shim: an unrecognised size is a broken file, and
        // `load_or_default` falls back to defaults rather than guessing.
        for input in ["\"huge\"", "\"\"", "280"] {
            assert!(
                serde_json::from_str::<WheelSize>(input).is_err(),
                "expected {input} to be rejected"
            );
        }
    }

    #[test]
    fn test_unknown_output_policy_does_not_wipe_sibling_settings() {
        // An unknown policy string falls back to the default policy, but the
        // rest of the file is still read, so one bad value cannot silently reset
        // every other preference.
        let json = r#"{
            "version": 2,
            "general": { "launch_at_login": true, "language": "fr",
                         "auto_update": false, "minimize_to_tray": true,
                         "include_prereleases": true },
            "output": {
                "policy": "something_lime_never_wrote",
                "fixed_folder": "C:/Users/test/Out",
                "suffix": "_x",
                "recycle_source": true,
                "metadata": { "images": true, "audio": false, "video": true }
            }
        }"#;

        let parsed: WheelSettings = serde_json::from_str(json).expect("file must parse");
        assert_eq!(parsed.output.policy, OutputPolicy::NextToSource);
        // Sibling settings must be intact, which is the whole point.
        assert_eq!(parsed.output.fixed_folder.as_deref(), Some("C:/Users/test/Out"));
        assert!(parsed.output.recycle_source);
        assert!(parsed.general.launch_at_login);
        assert_eq!(parsed.general.language, "fr");
    }

    #[test]
    fn test_output_policy_round_trips_and_defaults() {
        for policy in [
            OutputPolicy::NextToSource,
            OutputPolicy::FixedFolder,
            OutputPolicy::Clipboard,
        ] {
            let json = serde_json::to_string(&policy).unwrap();
            assert_eq!(serde_json::from_str::<OutputPolicy>(&json).unwrap(), policy);
        }
        assert_eq!(OutputPolicy::default(), OutputPolicy::NextToSource);
        // Unknown values fall back rather than failing the file.
        assert_eq!(
            serde_json::from_str::<OutputPolicy>("\"something_new\"").unwrap(),
            OutputPolicy::NextToSource
        );
    }

    #[test]
    fn test_first_run_detection_uses_readability_not_file_presence() {
        // The distinction matters: a readable config means the user has been
        // here before and must not be shown onboarding, but a corrupt or missing
        // one should not strand them either.
        let dir = std::env::temp_dir().join("wheel_settings_first_run");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");

        // No file at all: genuine fresh install.
        let _ = std::fs::remove_file(&path);
        let (settings, first_run) = WheelSettings::load_reporting_first_run(&path);
        assert!(first_run);
        assert!(!settings.general.onboarding_completed);

        // A readable config from before the onboarding field existed: still an
        // upgrade, so onboarding stays out of the way.
        std::fs::write(
            &path,
            r#"{"version":1,"general":{"launch_at_login":false,"language":"en",
               "auto_update":true,"minimize_to_tray":true,"include_prereleases":true}}"#,
        )
        .unwrap();
        let (_, first_run) = WheelSettings::load_reporting_first_run(&path);
        assert!(!first_run, "an existing config must not trigger onboarding");

        // Same for a config at the current schema version.
        let current = WheelSettings::default();
        current.save_to_path(&path).unwrap();
        let (_, first_run) = WheelSettings::load_reporting_first_run(&path);
        assert!(!first_run);

        // And for one the user finished onboarding on.
        let mut finished = WheelSettings::default();
        finished.general.onboarding_completed = true;
        finished.save_to_path(&path).unwrap();
        let (loaded, first_run) = WheelSettings::load_reporting_first_run(&path);
        assert!(!first_run);
        assert!(loaded.general.onboarding_completed);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_settings_file_is_treated_as_a_fresh_install() {
        // Better to show onboarding than to leave a user with no way to reach it.
        let dir = std::env::temp_dir().join("wheel_settings_corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        std::fs::write(&path, b"{ not json at all").unwrap();

        let (settings, first_run) = WheelSettings::load_reporting_first_run(&path);
        assert!(first_run);
        assert_eq!(settings, WheelSettings::default());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_default_settings_validity() {
        let settings = WheelSettings::default();
        assert_eq!(settings.version, SETTINGS_VERSION);
        assert_eq!(settings.trigger.modifier, TriggerModifier::Shift);
        assert_eq!(settings.trigger.movement_threshold_px, 6);
        assert!(!settings.trigger.paused);
        assert_eq!(settings.output.policy, OutputPolicy::NextToSource);
        assert!(!settings.output.overwrite_source);
    }

    #[test]
    fn test_settings_serde_roundtrip() {
        let original = WheelSettings::default();
        let json = serde_json::to_string(&original).expect("serialize settings");
        let parsed: WheelSettings = serde_json::from_str(&json).expect("deserialize settings");
        assert_eq!(parsed.version, original.version);
        assert_eq!(parsed.trigger.movement_threshold_px, 6);
    }

    #[test]
    fn test_version_is_stamped_on_load() {
        // There are no migrations while Lime is unreleased; a file written by an
        // older build is simply restamped to the current schema version.
        let dir = std::env::temp_dir().join("wheel_settings_version_stamp");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        std::fs::write(&path, br#"{ "version": 1, "general": { "language": "en" } }"#).unwrap();

        let loaded = WheelSettings::load_or_default(&path);
        assert_eq!(loaded.version, SETTINGS_VERSION);
        assert_eq!(loaded.general.language, "en");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_settings_partial_eq() {
        let s1 = WheelSettings::default();
        let mut s2 = WheelSettings::default();
        assert_eq!(s1, s2);

        s2.general.launch_at_login = true;
        assert_ne!(s1, s2);
        assert_ne!(s1.general.launch_at_login, s2.general.launch_at_login);
    }

    #[test]
    fn test_wheel_size_serializes_as_preset_name() {
        let json = serde_json::to_string(&WheelUiSettings::default()).expect("serialize wheel ui");
        assert!(json.contains("\"size\":\"medium\""), "got: {}", json);
    }

    #[test]
    fn test_wheel_size_deserializes_preset_names() {
        for (name, expected, px) in [
            ("small", WheelSize::Small, 280),
            ("medium", WheelSize::Medium, 320),
            ("large", WheelSize::Large, 400),
        ] {
            let parsed: WheelSize = serde_json::from_str(&format!("\"{}\"", name))
                .unwrap_or_else(|e| panic!("failed to parse {}: {}", name, e));
            assert_eq!(parsed, expected);
            assert_eq!(parsed.px(), px);
            // Every preset must fit the fixed 400x400 overlay window.
            assert!(parsed.px() <= 400, "{} exceeds the overlay window", name);
        }
    }

    #[test]
    fn test_a_sparse_settings_file_fills_in_defaults() {
        // Fields marked `#[serde(default)]` may be omitted from the JSON and fall
        // back to their default instead of failing the parse, so a settings file
        // written before a field existed is still usable.
        let sparse = r#"{
            "version": 2,
            "general": {
                "launch_at_login": true, "language": "fr",
                "auto_update": false, "minimize_to_tray": true,
                "include_prereleases": true
            },
            "wheel_ui": {
                "slot_count": 8, "theme": "system", "animation_speed": 1.0,
                "reduced_motion": true, "size": "large", "corner_radius": 16,
                "sound_enabled": false, "context_filter_enabled": true
            }
        }"#;
        let parsed: WheelSettings =
            serde_json::from_str(sparse).expect("a sparse file should deserialize");

        // What was written is kept.
        assert!(parsed.general.launch_at_login);
        assert_eq!(parsed.general.language, "fr");
        assert_eq!(parsed.wheel_ui.size, WheelSize::Large);

        // What was omitted falls back.
        assert_eq!(parsed.wheel_ui.status_vertical, StatusVertical::Bottom);
        assert_eq!(parsed.wheel_ui.status_horizontal, StatusHorizontal::Center);
        assert_eq!(parsed.wheel_ui.hud_style, HudStyle::Standard);
        assert_eq!(parsed.output.policy, OutputPolicy::NextToSource);
        assert!(parsed.output.metadata.images);
    }

    #[test]
    fn test_metadata_settings_default_preserves_all() {
        let out = OutputSettings::default();
        assert!(out.metadata.images && out.metadata.audio && out.metadata.video);
    }

    #[test]
    fn test_hud_style_roundtrips() {
        let mut ui = WheelUiSettings::default();
        ui.hud_style = HudStyle::Compact;
        let json = serde_json::to_string(&ui).expect("serialize wheel ui");
        assert!(json.contains("\"hud_style\":\"compact\""), "got: {}", json);
        let parsed: WheelUiSettings = serde_json::from_str(&json).expect("deserialize wheel ui");
        assert_eq!(parsed.hud_style, HudStyle::Compact);
    }

    #[test]
    fn test_status_anchors_span_the_work_area() {
        assert_eq!(StatusVertical::Top.anchor(), 0.0);
        assert_eq!(StatusVertical::Middle.anchor(), 0.5);
        assert_eq!(StatusVertical::Bottom.anchor(), 1.0);
        assert_eq!(StatusHorizontal::Left.anchor(), 0.0);
        assert_eq!(StatusHorizontal::Center.anchor(), 0.5);
        assert_eq!(StatusHorizontal::Right.anchor(), 1.0);
    }

    #[test]
    fn test_status_position_roundtrips() {
        let mut ui = WheelUiSettings::default();
        ui.status_vertical = StatusVertical::Top;
        ui.status_horizontal = StatusHorizontal::Right;
        let json = serde_json::to_string(&ui).expect("serialize wheel ui");
        let parsed: WheelUiSettings = serde_json::from_str(&json).expect("deserialize wheel ui");
        assert_eq!(parsed.status_vertical, StatusVertical::Top);
        assert_eq!(parsed.status_horizontal, StatusHorizontal::Right);
    }
}
