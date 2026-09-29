use serde::{Deserialize, Serialize};

/// Versioned settings schema.
/// Increment SETTINGS_VERSION when making breaking changes; add a migration.
pub const SETTINGS_VERSION: u32 = 1;

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
    #[serde(default = "default_presets")]
    pub presets: Vec<ActionPreset>,
}

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
            presets: default_presets(),
        }
    }
}

impl WheelSettings {
    /// Apply schema migrations to bring old settings up to the current version.
    pub fn migrate(mut self) -> Self {
        // Example migration: v0 -> v1
        // if self.version == 0 { ... self.version = 1; }
        self.version = SETTINGS_VERSION;
        if self.presets.is_empty() {
            self.presets = default_presets();
        }
        self
    }

    /// Load settings from a JSON file, or return default settings if the file does not exist.
    pub fn load_or_default<P: AsRef<std::path::Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(settings) = serde_json::from_str::<WheelSettings>(&content) {
                    return settings.migrate();
                }
            }
        }
        Self::default()
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
    pub explorer_context_menu: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            language: "en".into(),
            auto_update: true,
            minimize_to_tray: true,
            explorer_context_menu: true,
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
    pub size: u32,
    pub corner_radius: u32,
    pub sound_enabled: bool,
    pub context_filter_enabled: bool,
}

impl Default for WheelUiSettings {
    fn default() -> Self {
        Self {
            slot_count: 8,
            theme: Theme::System,
            animation_speed: 1.0,
            reduced_motion: false,
            size: 320,
            corner_radius: 16,
            sound_enabled: false,
            context_filter_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputPolicy {
    /// Save next to the source file (default)
    NextToSource,
    /// Save to a fixed folder
    FixedFolder,
    /// Ask every time (opens save dialog)
    AskEachTime,
    /// Copy result to clipboard
    Clipboard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutputSettings {
    pub policy: OutputPolicy,
    pub fixed_folder: Option<String>,
    /// Suffix added between stem and extension: e.g. ".converted" → file.converted.png
    pub suffix: String,
    /// Whether to overwrite the source file (default: false, very dangerous)
    pub overwrite_source: bool,
    /// Send source to Recycle Bin after a successful conversion
    pub recycle_source: bool,
    /// Preserve metadata in conversions
    pub preserve_metadata: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            policy: OutputPolicy::NextToSource,
            fixed_folder: None,
            suffix: String::new(),
            overwrite_source: false,
            recycle_source: false,
            preserve_metadata: true,
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
            id: "preset.clean_web".into(),
            name: "Clean Web Asset".into(),
            description: "Strip EXIF metadata and convert to WebP".into(),
            icon: "sparkles".into(),
            steps: vec![
                PresetStep { action_id: "tool.metadata".into(), params: serde_json::json!({ "strip": "all" }) },
                PresetStep { action_id: "convert.webp".into(), params: serde_json::json!({}) },
            ],
            enabled: true,
        },
        ActionPreset {
            id: "preset.share_screenshot".into(),
            name: "Share Screenshot".into(),
            description: "Add stylish background padding and export as PNG".into(),
            icon: "image".into(),
            steps: vec![
                PresetStep { action_id: "tool.addbg".into(), params: serde_json::json!({ "padding": 40 }) },
                PresetStep { action_id: "convert.png".into(), params: serde_json::json!({}) },
            ],
            enabled: true,
        },
        ActionPreset {
            id: "preset.transparent_png".into(),
            name: "Cutout PNG".into(),
            description: "Remove background and export as transparent PNG".into(),
            icon: "scissors".into(),
            steps: vec![
                PresetStep { action_id: "tool.removebg".into(), params: serde_json::json!({ "feather": 2 }) },
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
    fn test_settings_migration() {
        let mut settings = WheelSettings::default();
        settings.version = 0;
        let migrated = settings.migrate();
        assert_eq!(migrated.version, SETTINGS_VERSION);
    }

    #[test]
    fn test_settings_partial_eq() {
        let s1 = WheelSettings::default();
        let mut s2 = WheelSettings::default();
        assert_eq!(s1, s2);

        s2.general.launch_at_login = true;
        assert_ne!(s1, s2);
        assert_ne!(s1.general.launch_at_login, s2.general.launch_at_login);
        assert_eq!(s1.general.explorer_context_menu, s2.general.explorer_context_menu);
    }
}
