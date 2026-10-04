use serde::{Deserialize, Serialize};

/// The display name of the application. Change only this constant to rename.
pub const APP_NAME: &str = "Lime";
pub const APP_ID: &str = "com.limeapp.lime";

/// Unique identifier for an action (e.g. "convert.png", "tool.crop")
pub type ActionId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionCategory {
    Convert,
    Tools,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Runs in the background; no window opened
    Instant,
    /// Opens a dedicated tool window
    Window,
}

/// Which file types and cardinality an action accepts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedInput {
    /// File extensions this action accepts (lowercase, no dot). Empty = accept all.
    pub extensions: Vec<String>,
    /// Whether this action accepts multiple files at once
    pub multi: bool,
}

impl AcceptedInput {
    pub fn accepts_extension(&self, ext: &str) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        let clean = ext.trim_start_matches('.').to_lowercase();
        self.extensions.iter().any(|e| e.to_lowercase() == clean)
    }
}

/// Window configuration for tool-kind actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub mica: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 640,
            resizable: true,
            mica: true,
        }
    }
}

/// The static manifest that every action must provide
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionManifest {
    pub id: ActionId,
    pub title: String,
    /// Icon key (from the icon set) or a filesystem path
    pub icon: String,
    pub category: ActionCategory,
    pub accepts: AcceptedInput,
    pub kind: ActionKind,
    /// Only relevant when kind == Window
    pub window: Option<WindowConfig>,
    /// Default parameter values (format-specific, tool-specific)
    #[serde(default)]
    pub defaults: serde_json::Value,
    /// Whether this action is enabled by default
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Sort order within its page
    #[serde(default)]
    pub order: u32,
}

fn default_true() -> bool {
    true
}

impl ActionManifest {
    pub fn accepts_files(&self, extensions: &[&str]) -> bool {
        extensions.iter().any(|ext| self.accepts.accepts_extension(ext))
    }
}

/// The global action registry, populated at startup.
pub struct ActionRegistry {
    actions: Vec<ActionManifest>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn register(&mut self, manifest: ActionManifest) {
        self.actions.push(manifest);
    }

    pub fn get(&self, id: &str) -> Option<&ActionManifest> {
        self.actions.iter().find(|a| a.id == id)
    }

    pub fn all(&self) -> &[ActionManifest] {
        &self.actions
    }

    pub fn for_category<'a>(&'a self, cat: &'a ActionCategory) -> impl Iterator<Item = &'a ActionManifest> + 'a {
        self.actions.iter().filter(move |a| &a.category == cat && a.enabled)
    }

    /// Filter actions that accept at least one of the given extensions
    pub fn compatible<'a>(&'a self, extensions: &'a [&'a str]) -> impl Iterator<Item = &'a ActionManifest> + 'a {
        self.actions.iter().filter(move |a| {
            a.enabled && a.accepts_files(extensions)
        })
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default set of built-in actions
pub fn default_actions() -> Vec<ActionManifest> {
    let image_exts = vec![
        "png".into(), "jpg".into(), "jpeg".into(), "webp".into(),
        "avif".into(), "tiff".into(), "tif".into(),
        "bmp".into(), "gif".into(), "ico".into(), "svg".into(),
    ];
    let video_exts = vec![
        "mp4".into(), "mov".into(), "mkv".into(), "webm".into(), "avi".into(),
    ];
    let audio_exts = vec![
        "mp3".into(), "wav".into(), "m4a".into(), "flac".into(), "aac".into(),
    ];

    let mut actions = Vec::new();

    // --- Image Convert actions ---
    let convert_targets = [
        ("convert.png",  "PNG",  "png"),
        ("convert.webp", "WEBP", "webp"),
        ("convert.jpg",  "JPG",  "jpg"),
        ("convert.tiff", "TIFF", "tiff"),
        ("convert.avif", "AVIF", "avif"),
        ("convert.bmp",  "BMP",  "bmp"),
        ("convert.pdf",  "PDF",  "pdf"),
        ("convert.ico",  "ICO",  "ico"),
    ];

    for (i, (id, title, _fmt)) in convert_targets.iter().enumerate() {
        actions.push(ActionManifest {
            id: id.to_string(),
            title: title.to_string(),
            icon: format!("format-{}", id.split('.').nth(1).unwrap_or("file")),
            category: ActionCategory::Convert,
            accepts: AcceptedInput {
                extensions: image_exts.clone(),
                multi: true,
            },
            kind: ActionKind::Instant,
            window: None,
            defaults: serde_json::json!({}),
            enabled: true,
            order: i as u32,
        });
    }

    // --- Video Convert actions ---
    let video_targets = [
        ("convert.mp4",  "MP4",  "mp4"),
        ("convert.webm", "WEBM", "webm"),
        ("convert.gif",  "GIF",  "gif"),
        ("convert.mov",  "MOV",  "mov"),
        ("convert.mkv",  "MKV",  "mkv"),
        ("convert.avi",  "AVI",  "avi"),
    ];

    for (i, (id, title, _fmt)) in video_targets.iter().enumerate() {
        actions.push(ActionManifest {
            id: id.to_string(),
            title: title.to_string(),
            icon: format!("format-{}", id.split('.').nth(1).unwrap_or("file")),
            category: ActionCategory::Convert,
            accepts: AcceptedInput {
                extensions: video_exts.clone(),
                multi: true,
            },
            kind: ActionKind::Instant,
            window: None,
            defaults: serde_json::json!({}),
            enabled: true,
            order: (convert_targets.len() + i) as u32,
        });
    }

    // --- Audio Convert / Extract actions ---
    let audio_targets = [
        ("convert.mp3",  "MP3",  "mp3"),
        ("convert.wav",  "WAV",  "wav"),
        ("convert.m4a",  "M4A",  "m4a"),
        ("convert.flac", "FLAC", "flac"),
        ("convert.aac",  "AAC",  "aac"),
        ("convert.ogg",  "OGG",  "ogg"),
        ("convert.opus", "OPUS", "opus"),
        ("convert.wma",  "WMA",  "wma"),
    ];

    let mut media_for_audio = video_exts.clone();
    media_for_audio.extend(audio_exts.clone());

    for (i, (id, title, _fmt)) in audio_targets.iter().enumerate() {
        actions.push(ActionManifest {
            id: id.to_string(),
            title: title.to_string(),
            icon: format!("format-{}", id.split('.').nth(1).unwrap_or("file")),
            category: ActionCategory::Convert,
            accepts: AcceptedInput {
                extensions: media_for_audio.clone(),
                multi: true,
            },
            kind: ActionKind::Instant,
            window: None,
            defaults: serde_json::json!({}),
            enabled: true,
            order: (convert_targets.len() + video_targets.len() + i) as u32,
        });
    }

    // --- Tool actions ---
    let transparent_image_exts = vec![
        "png".into(), "webp".into(), "avif".into(), "tiff".into(), "tif".into(),
        "gif".into(), "ico".into(), "svg".into(), "heic".into(),
    ];

    actions.push(ActionManifest {
        id: "tool.trim".to_string(),
        title: "TRIM".to_string(),
        icon: "trim".to_string(),
        category: ActionCategory::Tools,
        accepts: AcceptedInput {
            extensions: transparent_image_exts,
            multi: true,
        },
        kind: ActionKind::Instant,
        window: None,
        defaults: serde_json::json!({}),
        enabled: true,
        order: 0,
    });

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_actions_populated() {
        let actions = default_actions();
        let convert_count = actions.iter().filter(|a| a.category == ActionCategory::Convert).count();
        let tools_count = actions.iter().filter(|a| a.category == ActionCategory::Tools).count();
        assert!(convert_count >= 8);
        assert_eq!(tools_count, 1);
    }

    #[test]
    fn test_action_registry_lookup_and_filter() {
        let mut registry = ActionRegistry::new();
        for action in default_actions() {
            registry.register(action);
        }

        assert!(registry.get("convert.png").is_some());
        assert!(registry.get("tool.trim").is_some());
        assert!(registry.get("convert.mp4").is_some());
        assert!(registry.get("convert.mp3").is_some());
        assert!(registry.get("nonexistent").is_none());

        let converts: Vec<_> = registry.for_category(&ActionCategory::Convert).collect();
        assert!(converts.len() >= 8);

        let image_actions: Vec<_> = registry.compatible(&["png"]).collect();
        assert!(!image_actions.is_empty());

        let video_only_actions: Vec<_> = registry.compatible(&["mp4"]).collect();
        assert!(!video_only_actions.is_empty());

        let audio_only_actions: Vec<_> = registry.compatible(&["mp3"]).collect();
        assert!(!audio_only_actions.is_empty());

        let trim_action = registry.get("tool.trim").unwrap();
        assert!(trim_action.accepts.accepts_extension("png"));
        assert!(!trim_action.accepts.accepts_extension("jpg"));

        let pdf_actions: Vec<_> = registry.compatible(&["pdf"]).collect();
        assert_eq!(pdf_actions.len(), 0);

        let convert_pdf = registry.get("convert.pdf").unwrap();
        assert!(convert_pdf.accepts.accepts_extension("png"));
        assert!(convert_pdf.accepts.accepts_extension("jpg"));
        assert!(!convert_pdf.accepts.accepts_extension("pdf"));
    }

    #[test]
    fn test_accepted_input_matching() {
        let input = AcceptedInput {
            extensions: vec!["png".into(), "jpg".into()],
            multi: false,
        };
        assert!(input.accepts_extension("png"));
        assert!(input.accepts_extension("PNG"));
        assert!(input.accepts_extension(".png"));
        assert!(!input.accepts_extension("gif"));

        let accept_all = AcceptedInput {
            extensions: vec![],
            multi: true,
        };
        assert!(accept_all.accepts_extension("any"));
    }
}
