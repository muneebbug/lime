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
    //
    // Two different questions, and conflating them is what made every tool
    // vanish for a JPEG.
    //
    // Trim removes blank margins, which only exist where there is an alpha
    // channel to be blank. A JPEG has no transparency, so there is nothing for
    // it to find and it is correctly unavailable there.
    //
    // SVG is excluded for a different reason: the engine rasterises it before
    // cropping, then can only write the result back as a raster format, so saving
    // to a `.svg` path failed with "Failed to save trimmed image". Trimming a
    // vector properly means rewriting its viewBox, which is a feature rather than
    // a bug fix. Recolor still handles SVG, because it edits the markup.
    //
    // Recolor repaints colour, and a JPEG is full of it. It was previously given
    // the transparent-only list, which meant the wheel filtered every tool out
    // for a .jpg: the Tools page came up empty and nothing could be hovered.
    // The engine never had that limitation - it decodes and re-encodes JPEG and
    // BMP, and warns on save that they cannot store transparency.
    let transparent_image_exts = vec![
        "png".into(), "webp".into(), "avif".into(), "tiff".into(), "tif".into(),
        "gif".into(), "ico".into(), "heic".into(),
    ];
    let opaque_image_exts = vec![
        "jpg".into(), "jpeg".into(), "bmp".into(),
    ];

    // Recolor additionally takes SVG, which it edits as source rather than
    // rasterising.
    let mut all_image_exts = transparent_image_exts.clone();
    all_image_exts.extend(opaque_image_exts);
    all_image_exts.push("svg".into());

    actions.push(ActionManifest {
        id: "tool.trim".to_string(),
        title: "TRIM".to_string(),
        icon: "trim".to_string(),
        category: ActionCategory::Tools,
        accepts: AcceptedInput {
            extensions: transparent_image_exts.clone(),
            multi: true,
        },
        kind: ActionKind::Instant,
        window: None,
        defaults: serde_json::json!({}),
        enabled: true,
        order: 0,
    });

    // Recolor opens its own window: it needs a side-by-side preview and a live
    // rule list, which a progress HUD cannot express. The window is wider and
    // taller than the default because the layout is two panes.
    actions.push(ActionManifest {
        id: "tool.recolor".to_string(),
        title: "Recolor Image".to_string(),
        icon: "recolor".to_string(),
        category: ActionCategory::Tools,
        accepts: AcceptedInput {
            // Every image format, alpha or not.
            extensions: all_image_exts,
            // One image at a time. The window edits a single file, and
            // "replace blue with red" is a decision about one image.
            multi: false,
        },
        kind: ActionKind::Window,
        window: Some(WindowConfig {
            width: 1180,
            height: 780,
            resizable: true,
            mica: true,
        }),
        defaults: serde_json::json!({}),
        enabled: true,
        order: 1,
    });

    // Compress: only formats where compressing is a real saving.
    //
    // Deliberately narrower than the set the app can open. GIF is out because
    // its compression path is partial and re-encoding an animation risks
    // flattening it to one frame, which is data loss. SVG would have to be
    // rasterised to shrink, the same mistake as trimming one. ICO is already
    // packed. HEIC cannot be decoded here at all, and AVIF is already
    // compressed, so re-encoding buys nothing.
    let compressible_exts = vec![
        "jpg".into(), "jpeg".into(), "png".into(), "webp".into(),
        "tiff".into(), "tif".into(),
    ];

    // Compress opens its own window: it shows a before/after size comparison and
    // per-format settings, and a progress bar for the slow presets.
    actions.push(ActionManifest {
        id: "tool.compress".to_string(),
        title: "Compress Image".to_string(),
        icon: "compress".to_string(),
        category: ActionCategory::Tools,
        accepts: AcceptedInput {
            extensions: compressible_exts,
            // One at a time. The window reports a single before/after pair, and
            // a batch here would need a different design rather than a loop.
            multi: false,
        },
        kind: ActionKind::Window,
        window: Some(WindowConfig {
            width: 860,
            height: 720,
            resizable: true,
            mica: true,
        }),
        defaults: serde_json::json!({}),
        enabled: true,
        order: 2,
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
        assert_eq!(tools_count, 3);
    }

    /// Tools available for one file extension, in registry order.
    fn tools_for(ext: &str) -> Vec<String> {
        let actions = default_actions();
        actions
            .iter()
            .filter(|a| a.category == ActionCategory::Tools)
            .filter(|a| a.accepts.accepts_extension(ext))
            .map(|a| a.id.clone())
            .collect()
    }

    #[test]
    fn compress_offers_only_formats_where_it_saves_bytes() {
        for ext in ["jpg", "jpeg", "png", "webp", "tiff"] {
            assert!(
                tools_for(ext).contains(&"tool.compress".to_string()),
                "{ext} should offer Compress"
            );
        }
    }

    #[test]
    fn compress_refuses_formats_where_it_would_lose_data_or_do_nothing() {
        let actions = default_actions();
        let compress = actions
            .iter()
            .find(|a| a.id == "tool.compress")
            .expect("compress must exist");

        // GIF: partial support risks flattening an animation to one frame.
        assert!(!compress.accepts.accepts_extension("gif"), "gif must be refused");
        // SVG: shrinking it means rasterising it.
        assert!(!compress.accepts.accepts_extension("svg"), "svg must be refused");
        // Already packed or already compressed.
        for ext in ["ico", "avif", "heic", "bmp"] {
            assert!(
                !compress.accepts.accepts_extension(ext),
                "{ext} must be refused"
            );
        }
    }

    #[test]
    fn compress_matches_the_engines_own_whitelist() {
        // The engine's own list is checked against this in
        // `tests/compress_manifest_agrees.rs`, which lives in wheel-engines
        // because the dependency runs that way and a core test cannot see it.
        let actions = default_actions();
        let compress = actions
            .iter()
            .find(|a| a.id == "tool.compress")
            .expect("compress must exist");
        assert!(!compress.accepts.extensions.is_empty());
    }

    #[test]
    fn opaque_images_offer_recolor() {
        // The bug this pins: every tool was gated on the transparent-only list,
        // so a JPEG produced an empty Tools page and nothing could be hovered.
        // Compress covers JPEG too; BMP has no alpha to trim and no compression
        // worth doing, so it only gets Recolor.
        for ext in ["jpg", "jpeg"] {
            assert_eq!(
                tools_for(ext),
                vec!["tool.recolor".to_string(), "tool.compress".to_string()],
                "{ext} should offer Recolor and Compress"
            );
        }
        assert_eq!(
            tools_for("bmp"),
            vec!["tool.recolor".to_string()],
            "bmp has neither alpha to trim nor a useful compression"
        );
    }

    #[test]
    fn transparent_images_offer_trim_and_recolor() {
        // PNG, WebP and TIFF compress as well. GIF does not: re-encoding an
        // animation risks flattening it to one frame.
        for ext in ["png", "webp", "tiff", "tif"] {
            assert_eq!(
                tools_for(ext),
                vec![
                    "tool.trim".to_string(),
                    "tool.recolor".to_string(),
                    "tool.compress".to_string()
                ],
                "{ext} should offer all three tools"
            );
        }

        for ext in ["gif", "avif", "ico", "heic"] {
            assert_eq!(
                tools_for(ext),
                vec!["tool.trim".to_string(), "tool.recolor".to_string()],
                "{ext} should offer Trim and Recolor only"
            );
        }
    }

    #[test]
    fn svg_offers_recolor_only() {
        // Trimming rasterises the SVG and then cannot write it back as a vector,
        // so the wheel must not offer it. Recolor edits the markup and works.
        assert_eq!(tools_for("svg"), vec!["tool.recolor".to_string()]);
    }

    #[test]
    fn trim_stays_away_from_formats_with_no_alpha() {
        // Trimming finds blank margins, and a JPEG has no blank margins to find.
        let actions = default_actions();
        let trim = actions
            .iter()
            .find(|a| a.id == "tool.trim")
            .expect("trim must exist");

        for ext in ["jpg", "jpeg", "bmp"] {
            assert!(
                !trim.accepts.accepts_extension(ext),
                "trim must not claim {ext}"
            );
        }
        // SVG is trimmed by rasterising, which cannot be written back as a
        // vector, so the wheel must not offer it either.
        assert!(
            !trim.accepts.accepts_extension("svg"),
            "trim must not claim svg"
        );
    }

    #[test]
    fn recolor_covers_every_image_format_the_wheel_offers() {
        let actions = default_actions();
        let recolor = actions
            .iter()
            .find(|a| a.id == "tool.recolor")
            .expect("recolor must exist");

        for ext in [
            "png", "webp", "avif", "tiff", "tif", "gif", "ico", "svg", "heic", "jpg", "jpeg",
            "bmp",
        ] {
            assert!(
                recolor.accepts.accepts_extension(ext),
                "recolor must accept {ext}"
            );
        }
    }

    #[test]
    fn recolor_is_registered_as_a_single_image_tool_window() {
        let registry = default_actions()
            .into_iter()
            .fold(ActionRegistry::new(), |mut acc, action| {
                acc.register(action);
                acc
            });

        let recolor = registry.get("tool.recolor").expect("recolor must exist");

        assert_eq!(recolor.kind, ActionKind::Window);
        assert_eq!(recolor.category, ActionCategory::Tools);
        assert!(!recolor.accepts.multi, "recolor edits one image at a time");
        assert!(recolor.accepts.extensions.contains(&"png".to_string()));
        assert!(recolor.accepts.extensions.contains(&"webp".to_string()));

        // The window config has to allow the two-pane layout to breathe, and has
        // to be resizable because previews are shown at different zooms.
        let window = recolor.window.as_ref().expect("a window config is required");
        assert!(window.width >= 900 && window.height >= 600);
        assert!(window.resizable);

        // Every accepted extension must be something the image engine can open,
        // otherwise the tool appears in the wheel and then fails to load.
        for ext in &recolor.accepts.extensions {
            assert!(
                wheel_engines_supported(ext),
                "recolor accepts .{ext}, which the image engine cannot decode"
            );
        }
    }

    /// Mirrors what `image_convert::load_image` can open, for the assertion
    /// above. Kept as a list rather than a call into the engine so this crate
    /// does not take a dependency on the engines crate, which sits above it.
    fn wheel_engines_supported(ext: &str) -> bool {
        matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "avif"
                | "tiff"
                | "tif"
                | "gif"
                | "ico"
                | "bmp"
                | "svg"
                | "svgz"
                | "heic"
                | "heif"
        )
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
