use serde::{Deserialize, Serialize};

/// The display name of the application. Change only this constant to rename.
pub const APP_NAME: &str = "Wheel";
pub const APP_ID: &str = "com.aediant.wheel";

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
        self.extensions.iter().any(|e| e == ext)
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

    pub fn for_category(&self, cat: &ActionCategory) -> impl Iterator<Item = &ActionManifest> {
        self.actions.iter().filter(move |a| &a.category == cat && a.enabled)
    }

    /// Filter actions that accept at least one of the given extensions
    pub fn compatible(&self, extensions: &[&str]) -> impl Iterator<Item = &ActionManifest> {
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
        "heic".into(), "avif".into(), "tiff".into(), "tif".into(),
        "bmp".into(), "gif".into(),
    ];

    let mut actions = Vec::new();

    // --- Convert actions ---
    let convert_targets = [
        ("convert.png",  "PNG",  "png"),
        ("convert.jpg",  "JPG",  "jpg"),
        ("convert.webp", "WEBP", "webp"),
        ("convert.avif", "AVIF", "avif"),
        ("convert.tiff", "TIFF", "tiff"),
        ("convert.bmp",  "BMP",  "bmp"),
        ("convert.heic", "HEIC", "heic"),
        ("convert.pdf",  "PDF",  "pdf"),
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

    // --- Tool actions ---
    let tools = [
        ("tool.crop",      "Crop",       "crop",       640u32,  860u32),
        ("tool.compress",  "Compress",   "compress",   580,     480),
        ("tool.metadata",  "Metadata",   "metadata",   680,     700),
        ("tool.addbg",     "Add BG",     "add-bg",     720,     820),
        ("tool.removebg",  "Remove BG",  "remove-bg",  720,     600),
        ("tool.edit",      "Edit",       "edit",       720,     840),
        ("tool.annotate",  "Annotate",   "annotate",   800,     700),
        ("tool.redact",    "Redact",     "redact",     800,     700),
    ];

    for (i, (id, title, icon, w, h)) in tools.iter().enumerate() {
        actions.push(ActionManifest {
            id: id.to_string(),
            title: title.to_string(),
            icon: icon.to_string(),
            category: ActionCategory::Tools,
            accepts: AcceptedInput {
                extensions: image_exts.clone(),
                multi: false,
            },
            kind: ActionKind::Window,
            window: Some(WindowConfig {
                width: *w,
                height: *h,
                resizable: true,
                mica: true,
            }),
            defaults: serde_json::json!({}),
            enabled: true,
            order: i as u32,
        });
    }

    actions
}
