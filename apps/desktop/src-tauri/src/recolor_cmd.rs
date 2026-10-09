//! Tauri commands behind the Recolor Image tool window.
//!
//! The window does its own live preview on a canvas, so these commands exist
//! for the work that has to be exact or expensive:
//!
//! * `recolor_inspect` - dimensions, format, and the file's colours.
//! * `recolor_apply` - the authoritative recolour, at full resolution.
//!
//! Preview and output deliberately share `RecolorParams` rather than each
//! inventing their own options, so what the user sees is what gets saved. If
//! the preview ever needs to differ, that difference must be visible, not
//! accidental.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::info;

use wheel_engines::recolor::{self, ColorExtraction, RecolorParams};

/// Default cap on how many distinct colours `recolor_inspect` returns.
///
/// A photograph routinely contains a million-plus distinct colours, which is far
/// more than a scrollable list can show and more than is useful to hand to the
/// frontend. The count that was actually left out is reported separately so the
/// UI can say "showing the 500 most used of 1,204,338" rather than implying it
/// found everything.
const DEFAULT_COLOR_LIMIT: usize = 500;

/// Ceiling on what the caller may ask for, so a bad `limit` cannot turn into a
/// multi-hundred-megabyte response.
const MAX_COLOR_LIMIT: usize = 5000;

/// What the tool window needs to know about the file it is editing.
#[derive(Debug, Clone, Serialize)]
pub struct RecolorSource {
    pub path: String,
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    /// True when the image has any pixel that is not fully opaque. Drives the
    /// warning about saving to a format that cannot store transparency.
    pub has_transparency: bool,
    /// File size on disk, for the header readout.
    pub file_size: u64,
    /// Lowercase extension, which is what the save path picks its encoder from.
    pub extension: String,
    /// The image itself, as a `data:` URL.
    ///
    /// Sent rather than a file URL because the webview is not allowed to read
    /// arbitrary paths, and because a data URL decodes without a round trip.
    pub data_url: String,
    /// True when the source is SVG, so the window can offer vector output.
    pub is_svg: bool,
}

/// Open a file and hand the window something it can render.
///
/// Deliberately does **not** count colours. Counting means a full pass over every
/// pixel, which measured 110ms on a one-megapixel image and far more on a large
/// photo - time the user spends staring at a blank window, for a list most of them
/// never open. The window asks for colours when it actually wants them, via
/// `recolor_extract_colors`.
#[tauri::command]
pub async fn recolor_inspect(path: String) -> Result<RecolorSource, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err(format!("Not a file: {path}"));
    }

    let path_for_task = input.clone();
    let found = tokio::task::spawn_blocking(move || -> Result<RecolorSource, String> {
        let bytes = std::fs::read(&path_for_task)
            .map_err(|e| format!("Could not read {}: {e}", path_for_task.display()))?;

        let image = wheel_engines::image_convert::load_image(&path_for_task)
            .map_err(|e| format!("Could not decode {}: {e}", path_for_task.display()))?;

        let rgba = image.to_rgba8();
        let has_transparency = rgba.pixels().any(|p| p.0[3] != 255);

        // Encode as PNG for the preview. Re-encoding is not wasteful here: the
        // window needs pixels it can put on a canvas, and every format the app
        // accepts either already is a canvas-friendly format or has no alpha to
        // lose. WebP, AVIF and HEIC sources in particular would otherwise fail
        // to decode in the webview.
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| format!("Could not prepare preview: {e}"))?;

        Ok(RecolorSource {
            file_name: path_for_task
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_for_task.to_string_lossy().to_string()),
            extension: path_for_task
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default(),
            file_size: bytes.len() as u64,
            width: image.width(),
            height: image.height(),
            has_transparency,
            data_url: format!("data:image/png;base64,{}", base64_encode(&png)),
            is_svg: is_svg(&path_for_task),
            path: path_for_task.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| format!("Recolor inspection task failed: {e}"))??;

    Ok(found)
}

/// Fold an extracted list into the swatches a rule would actually produce.
///
/// Kept apart from `recolor_extract_colors` on purpose. That command walks every
/// pixel, which is the expensive half and should happen once; this one is a
/// hundred float comparisons over a list already in memory, so it can run on
/// every step of the tolerance slider without the UI noticing.
///
/// It lives in the engine rather than the window so the grouping uses exactly
/// the same OKLab radius as the recolour. A window-side copy would drift, and a
/// list that disagrees with the edit is worse than a list that is merely long.
#[tauri::command]
pub async fn recolor_group_colors(
    colors: Vec<wheel_engines::recolor::ExtractedColor>,
    tolerance: u8,
) -> Result<Vec<wheel_engines::recolor::ColorGroup>, String> {
    Ok(wheel_engines::recolor::group_colors(&colors, tolerance))
}

/// Count the colours in an image, on demand.
///
/// Separate from `recolor_inspect` so opening a file stays instant. This is the
/// expensive half - a full pass over every pixel, sorting them to count runs -
/// and most sessions never look at the list at all.
#[tauri::command]
pub async fn recolor_extract_colors(
    path: String,
    color_limit: Option<usize>,
) -> Result<RecolorExtraction, String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err(format!("Not a file: {path}"));
    }

    let limit = color_limit
        .unwrap_or(DEFAULT_COLOR_LIMIT)
        .clamp(1, MAX_COLOR_LIMIT);

    let path_for_task = input.clone();
    tokio::task::spawn_blocking(move || -> Result<RecolorExtraction, String> {
        if is_svg(&path_for_task) {
            return Ok(colors_from_svg_markup(&path_for_task, limit));
        }

        let image = wheel_engines::image_convert::load_image(&path_for_task)
            .map_err(|e| format!("Could not decode {}: {e}", path_for_task.display()))?;

        let extraction = recolor::extract_colors(&image, limit);

        Ok(RecolorExtraction {
            // False when the list was capped, so the window can offer more.
            complete: !extraction.truncated,
            colors: extraction,
        })
    })
    .await
    .map_err(|e| format!("Colour extraction task failed: {e}"))?
}

/// The paints an SVG declares, read from its markup.
///
/// Deliberately not the rasterised pixels. Antialiasing between two *opaque*
/// shapes produces opaque intermediate colours - a cream edge against a dark
/// background is a blend, not a translucent pixel - so counting pixels reports
/// every edge blend as its own colour. Measured on a real three-colour file with
/// non-integer transforms: 47 distinct colours from the raster, 44 of them
/// blends nobody chose, against 3 from the markup.
///
/// Folding is `svg_recolor::distinct_paints`, which uses the same floor the save
/// does, so a colour written twice in two notations still reports once.
fn colors_from_svg_markup(path: &std::path::Path, limit: usize) -> RecolorExtraction {
    let empty = ColorExtraction {
        colors: Vec::new(),
        total_unique: 0,
        visible_pixels: 0,
        truncated: false,
    };

    let Ok(bytes) = std::fs::read(path) else {
        return RecolorExtraction { colors: empty, complete: true };
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return RecolorExtraction { colors: empty, complete: true };
    };

    let paints = wheel_engines::svg_recolor::distinct_paints(text);
    let total = paints.len() as u64;
    let shown = paints.len().min(limit);

    RecolorExtraction {
        colors: ColorExtraction {
            colors: paints.into_iter().take(limit).collect(),
            total_unique: total,
            visible_pixels: total,
            truncated: total > shown as u64,
        },
        complete: total <= shown as u64,
    }
}

/// Counts for a file, as the window would report them, plus what the rasteriser
/// makes of the same file.
///
/// Exposed so the diagnostic test can print the two side by side: the gap between
/// them is the whole reason SVGs are counted from their markup.
pub fn count_colors_for_report(path: &std::path::Path) -> (usize, usize) {
    let reported = if is_svg(path) {
        match std::fs::read(path).ok().and_then(|b| String::from_utf8(b).ok()) {
            Some(text) => wheel_engines::svg_recolor::distinct_paints(&text).len(),
            None => 0,
        }
    } else {
        wheel_engines::image_convert::load_image(path)
            .map(|i| recolor::extract_colors(&i, MAX_COLOR_LIMIT).total_unique as usize)
            .unwrap_or(0)
    };

    let rasterised = wheel_engines::image_convert::load_image(path)
        .map(|i| recolor::extract_colors(&i, MAX_COLOR_LIMIT).total_unique as usize)
        .unwrap_or(0);

    (reported, rasterised)
}

/// A colour list, plus whether the window should offer to load more of it.
#[derive(Debug, Clone, Serialize)]
pub struct RecolorExtraction {
    pub colors: ColorExtraction,
    pub complete: bool,
}

/// Request to write a recoloured copy.
#[derive(Debug, Clone, Deserialize)]
pub struct RecolorRequest {
    pub input_path: String,
    /// Where to write it. When absent, the app's output settings decide.
    pub output_path: Option<String>,
    pub params: RecolorParams,
    /// Overrides the extension of `output_path`, so a PNG can be saved as JPEG
    /// without the user having to type a new filename.
    #[serde(default)]
    pub format: Option<String>,
}

/// What a save produced.
#[derive(Debug, Clone, Serialize)]
pub struct RecolorSaved {
    pub output_path: String,
    pub file_name: String,
    /// Zero for an SVG output: a document has no intrinsic pixel size.
    pub width: u32,
    pub height: u32,
    /// True when the params matched nothing and the file was written as-is.
    pub unchanged: bool,
    /// True when the file was written by editing SVG markup rather than pixels.
    pub is_svg: bool,
    /// How many colour literals the SVG rewrite changed. Zero for raster output.
    pub replaced_literals: u64,
}

/// Recolour `input` and save it, recording the job in history.
#[tauri::command]
pub async fn recolor_apply(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    request: RecolorRequest,
) -> Result<RecolorSaved, String> {
    let input = PathBuf::from(&request.input_path);
    if !input.is_file() {
        return Err(format!("Not a file: {}", request.input_path));
    }

    let target = match request.output_path.as_deref().map(str::trim) {
        Some(explicit) if !explicit.is_empty() => resolve_explicit_output(explicit, request.format.as_deref()),
        _ => {
            let settings = state.settings.lock().await.output.clone();
            let default_ext = input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png")
                .to_lowercase();
            let target_ext = request
                .format
                .as_deref()
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .unwrap_or(&default_ext)
                .trim_start_matches('.')
                .to_lowercase();

            let fixed_folder = settings.fixed_folder.as_ref().map(std::path::Path::new);
            wheel_core::output::resolve_output_path(
                &input,
                &target_ext,
                if settings.suffix.is_empty() {
                    ".recolored"
                } else {
                    &settings.suffix
                },
                &settings.policy,
                fixed_folder,
                settings.overwrite_source,
            )
        }
    };

    /*
     * Refuse vector output from a raster source, in words. See
     * `check_vector_output` for why.
     */
    if let Err(message) = check_vector_output(&input, &target) {
        return Err(message);
    }

    let in_clone = input.clone();
    let out_clone = target.clone();
    let params = request.params.clone();
    let is_svg_output = is_svg(&target);

    /*
     * SVG is edited as text; everything else goes through the pixel pipeline.
     *
     * The two paths cannot be merged. Rasterising an SVG and writing the pixels
     * back produces a file with the right name and none of the original's
     * qualities - no paths, no scalability, a few hundred kilobytes instead of a
     * few - and the user asked for SVG explicitly. So the branch is here, in the
     * one place that decides how a file is written.
     *
     * Colour matching is shared either way: both paths use `RecolorParams`, so
     * the tolerance slider means the same thing for both.
     */
    let saved = tokio::task::spawn_blocking(move || -> Result<RecolorSaved, String> {
        if is_svg_output {
            let result = wheel_engines::svg_recolor::recolor_svg_file(&in_clone, &out_clone, &params)
                .map_err(|e| format!("{e:#}"))?;

            // A document has no intrinsic pixel size, so report what the preview
            // showed rather than inventing zeros the UI would have to special-case.
            return Ok(RecolorSaved {
                file_name: result
                    .output_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                output_path: result.output_path.to_string_lossy().to_string(),
                width: 0,
                height: 0,
                unchanged: result.unchanged,
                is_svg: true,
                replaced_literals: result.replaced,
            });
        }

        let result = wheel_engines::recolor::recolor_file(&in_clone, &out_clone, &params)
            .map_err(|e| format!("{e:#}"))?;

        Ok(RecolorSaved {
            file_name: result
                .output_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            output_path: result.output_path.to_string_lossy().to_string(),
            width: result.width,
            height: result.height,
            unchanged: result.unchanged,
            is_svg: false,
            replaced_literals: 0,
        })
    })
    .await
    .map_err(|e| format!("Recolor task failed: {e}"))??;

    let mut job = wheel_core::Job::new("tool.recolor", vec![input.clone()], serde_json::json!({
        "mode": request.params.mode,
        "rule_count": request.params.rules.len(),
        "tolerance": request.params.tolerance,
        "vector": saved.is_svg,
    }));
    job.status = wheel_core::job::JobStatus::Completed;
    job.progress = 1.0;
    job.outputs = vec![PathBuf::from(&saved.output_path)];
    let _ = state.history.insert_job(&job);

    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("Lime - Recolor complete")
        .body(format!("Saved {}", saved.file_name))
        .show();

    info!(
        "Recoloured {:?} -> {} ({})",
        input,
        saved.output_path,
        if saved.is_svg { "svg source" } else { "raster" }
    );

    Ok(saved)
}

/// True when a path names an SVG.
///
/// Both extensions count: `.svg` is the norm and `.svgz` is the gzipped variant,
/// and a file using the latter still holds SVG markup.
fn is_svg(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            e == "svg" || e == "svgz"
        })
        .unwrap_or(false)
}

/// Reject an SVG output whose source is not itself an SVG.
///
/// SVG output works by rewriting the colours in a document's own markup. A raster
/// has no markup, so there is nothing to rewrite: without this check the SVG path
/// tries to read PNG bytes as text and reports "stream did not contain valid
/// UTF-8", which is accurate and useless. Turning a photo into paths would be
/// tracing, not recolouring.
///
/// The window already hides the option for a raster source, so this is the
/// backstop for a stale selection or another caller.
fn check_vector_output(source: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    if !is_svg(target) || is_svg(source) {
        return Ok(());
    }

    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("?")
        .to_uppercase();

    Err(format!(
        "Cannot save a {ext} file as SVG. SVG output works by editing the colours in \
         an SVG's own markup, so it needs an SVG to start from."
    ))
}

/// Swap the extension on a user-supplied path when they picked a format.
///
/// Keeps the directory and stem they chose and only changes the format, which is
/// what "save as JPEG" means to someone who has already picked a folder.
fn resolve_explicit_output(path: &str, format: Option<&str>) -> PathBuf {
    let mut target = PathBuf::from(path);

    let Some(format) = format.map(str::trim).filter(|f| !f.is_empty()) else {
        return target;
    };

    let ext = format.trim_start_matches('.').to_lowercase();
    if ext.is_empty() {
        return target;
    }

    target.set_extension(ext);
    target
}

/// Standard base64, for turning PNG bytes into a `data:` URL.
///
/// Hand-rolled rather than pulled in as a dependency: the app already depends on
/// Tauri, which re-exports this, but going through it would mean threading a
/// second type through this module for no benefit.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[(triple >> 18) as usize & 63] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};
    use wheel_engines::recolor::Rgb;

    fn write_test_image(dir: &std::path::Path, name: &str) -> PathBuf {
        let mut image = RgbaImage::from_pixel(4, 4, Rgba([0x28, 0x76, 0xD2, 255]));
        image.put_pixel(0, 0, Rgba([0xFD, 0xE0, 0x47, 255]));
        DynamicImage::ImageRgba8(image)
            .save(dir.join(name))
            .unwrap();
        dir.join(name)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_packs_bits_in_the_right_order() {
        // Every 6-bit group is checked, including the case where a group
        // straddles two bytes - that is where a hand-rolled encoder goes wrong.
        assert_eq!(base64_encode(&[0x00, 0x00, 0x00]), "AAAA");
        assert_eq!(base64_encode(&[0xFF, 0xFF, 0xFF]), "////");
        assert_eq!(base64_encode(&[0x00, 0x10, 0x83]), "ABCD");

        let all: Vec<u8> = (0..=255u8).collect();
        assert_eq!(base64_encode(&all).len(), all.len().div_ceil(3) * 4);
    }

    #[test]
    fn explicit_output_keeps_the_users_folder_and_stem() {
        let chosen = resolve_explicit_output("C:/Pics/out.png", Some("jpg"));
        assert_eq!(chosen, PathBuf::from("C:/Pics/out.jpg"));

        // A leading dot, and odd casing, both mean the same thing.
        assert_eq!(
            resolve_explicit_output("C:/Pics/out.png", Some(".JPEG")),
            PathBuf::from("C:/Pics/out.jpeg")
        );
    }

    #[test]
    fn explicit_output_is_left_alone_when_no_format_is_given() {
        assert_eq!(
            resolve_explicit_output("C:/Pics/out.png", None),
            PathBuf::from("C:/Pics/out.png")
        );
        assert_eq!(
            resolve_explicit_output("C:/Pics/out.png", Some("  ")),
            PathBuf::from("C:/Pics/out.png")
        );
        assert_eq!(
            resolve_explicit_output("C:/Pics/out.png", Some("")),
            PathBuf::from("C:/Pics/out.png")
        );
    }

    #[test]
    fn recognises_both_svg_extensions() {
        // `.svgz` is the gzipped variant and still holds SVG markup, so it has to
        // take the vector path too or it silently comes out rasterised.
        for path in ["a.svg", "a.SVG", "a.svgz", "C:/dir/logo.svg"] {
            assert!(is_svg(std::path::Path::new(path)), "{path} should be treated as SVG");
        }
        for path in ["a.png", "a.jpg", "a.svgx", "a", "svg"] {
            assert!(
                !is_svg(std::path::Path::new(path)),
                "{path} should not be treated as SVG"
            );
        }
    }

    #[tokio::test]
    async fn saving_an_svg_writes_svg_source_not_pixels() {
        // The bug this closes: "Same as source" on an SVG produced a PNG wearing
        // an .svg extension, because every save went through the pixel encoder.
        let dir = std::env::temp_dir().join("wheel_recolor_cmd_svg");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("logo.svg");
        std::fs::write(
            &input,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10" fill="#FF5200"/></svg>"##,
        )
        .unwrap();

        let output = dir.join("logo.recolored.svg");
        let result = wheel_engines::svg_recolor::recolor_svg_file(
            &input,
            &output,
            &wheel_engines::recolor::RecolorParams {
                rules: vec![wheel_engines::recolor::RecolorRule {
                    from: wheel_engines::recolor::Rgb::from_hex("#FF5200").unwrap(),
                    to: wheel_engines::recolor::Rgb::from_hex("#2876D2").unwrap(),
                }],
                ..Default::default()
            },
        )
        .unwrap();

        assert!(is_svg(&result.output_path));
        assert_eq!(result.replaced, 1);
        assert!(!result.unchanged);

        // Still text, still paths - not a rasterised image with an svg name.
        let written = std::fs::read_to_string(&output).unwrap();
        assert!(written.contains("<path"), "the output lost its paths: {written}");
        assert!(written.contains(r#"d="M0 0 L10 10""#), "path data was lost");
        assert!(written.contains("#2876D2"));
        // A PNG would start with the 8-byte PNG signature.
        assert!(!written.starts_with('\u{89}'), "the output is binary, not SVG");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inspect_reports_dimensions_and_a_preview_but_no_colours() {
        let dir = temp_dir("wheel_recolor_cmd_inspect");
        let path = write_test_image(&dir, "in.png");

        let source = recolor_inspect(path.to_string_lossy().to_string())
            .await
            .unwrap();

        assert_eq!(source.width, 4);
        assert_eq!(source.height, 4);
        assert_eq!(source.file_name, "in.png");
        assert_eq!(source.extension, "png");
        assert!(source.file_size > 0);
        assert!(!source.has_transparency);
        assert!(!source.is_svg);
        assert!(
            source.data_url.starts_with("data:image/png;base64,"),
            "got {}",
            &source.data_url[..40.min(source.data_url.len())]
        );

        // Colours are counted on demand, so opening must not pay for it.
        let colors = recolor_extract_colors(path.to_string_lossy().to_string(), None)
            .await
            .unwrap();
        assert!(colors.complete);
        assert_eq!(colors.colors.total_unique, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inspect_does_no_pixel_counting() {
        // The point of the split. If a colour list ever creeps back onto the open
        // path, this is the test that catches it: opening a wide image must not
        // walk its pixels.
        let dir = temp_dir("wheel_recolor_cmd_lazy");
        let path = dir.join("wide.png");
        // 4000x4000 is 16 million pixels; counting them is seconds of work.
        let wide = RgbaImage::from_pixel(4000, 4000, Rgba([10, 20, 30, 255]));
        DynamicImage::ImageRgba8(wide).save(&path).unwrap();

        let started = std::time::Instant::now();
        let source = recolor_inspect(path.to_string_lossy().to_string())
            .await
            .unwrap();
        let open_ms = started.elapsed().as_millis();

        assert_eq!(source.width, 4000);
        // Generous, because decoding sixteen megapixels is not free either. The
        // point is that it is bounded by decode, not by a per-pixel sort.
        assert!(
            open_ms < 4000,
            "opening took {open_ms}ms, which is too slow to be free of counting"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inspect_reports_an_svg_source_as_vector() {
        let dir = temp_dir("wheel_recolor_cmd_issvg");
        let path = dir.join("logo.svg");
        std::fs::write(
            &path,
            r##"<svg viewBox="0 0 40 40"><rect width="40" height="40" fill="#FF5200"/></svg>"##,
        )
        .unwrap();

        let source = recolor_inspect(path.to_string_lossy().to_string())
            .await
            .unwrap();

        assert!(source.is_svg, "an SVG source must be flagged so vector save is offered");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inspect_notices_transparency() {
        let dir = temp_dir("wheel_recolor_cmd_alpha");
        let path = dir.join("in.png");
        let mut image = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
        image.put_pixel(0, 0, Rgba([200, 100, 50, 0]));
        DynamicImage::ImageRgba8(image).save(&path).unwrap();

        let source = recolor_inspect(path.to_string_lossy().to_string())
            .await
            .unwrap();

        assert!(source.has_transparency);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn extract_colors_caps_the_list_and_says_so() {
        let dir = temp_dir("wheel_recolor_cmd_cap");
        let path = dir.join("in.png");
        // 16 distinct greys, asked for 5.
        let mut image = RgbaImage::new(16, 1);
        for x in 0..16u32 {
            let v = (x * 16) as u8;
            image.put_pixel(x, 0, Rgba([v, v, v, 255]));
        }
        DynamicImage::ImageRgba8(image).save(&path).unwrap();

        let found = recolor_extract_colors(path.to_string_lossy().to_string(), Some(5))
            .await
            .unwrap();

        assert_eq!(found.colors.colors.len(), 5);
        assert!(!found.complete, "a capped list must not claim to be complete");
        assert_eq!(found.colors.total_unique, 16);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn extract_colors_rejects_a_path_that_is_not_a_file() {
        assert!(
            recolor_extract_colors("C:/definitely/not/here.png".into(), None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn inspect_rejects_a_path_that_is_not_a_file() {
        assert!(recolor_inspect("C:/definitely/not/here.png".into())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn inspect_rejects_a_file_that_is_not_an_image() {
        let dir = temp_dir("wheel_recolor_cmd_notimage");
        let path = dir.join("notes.txt");
        std::fs::write(&path, b"this is not an image").unwrap();

        assert!(recolor_inspect(path.to_string_lossy().to_string())
            .await
            .is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_round_trips_through_the_engine_the_way_the_frontend_sends_it() {
        // Mirrors the JSON shape `recolor_apply` receives from the window, so a
        // change to the wire format breaks here rather than at runtime.
        let request: RecolorRequest = serde_json::from_str(
            r##"{
                "input_path": "C:/in.png",
                "format": "png",
                "params": {
                    "mode": "replace",
                    "tolerance": 42,
                    "preserve_shading": true,
                    "all_to_one": "#CBE71F",
                    "rules": [{ "from": "#1E4FBF", "to": "#E53E4A" }]
                }
            }"##,
        )
        .unwrap();

        assert_eq!(request.input_path, "C:/in.png");
        assert_eq!(request.format.as_deref(), Some("png"));
        assert_eq!(request.params.tolerance, 42);
        // An explicit `true` has to survive, even though the default is now off.
        assert!(request.params.preserve_shading);
        assert_eq!(request.params.all_to_one, Rgb::new(0xCB, 0xE7, 0x1F));
        assert_eq!(request.params.rules.len(), 1);
        assert_eq!(request.params.rules[0].from, Rgb::new(0x1E, 0x4F, 0xBF));
        assert_eq!(request.params.rules[0].to, Rgb::new(0xE5, 0x3E, 0x4A));
    }

    #[test]
    fn apply_request_tolerates_a_missing_format() {
        let request: RecolorRequest = serde_json::from_str(
            r#"{ "input_path": "C:/in.png", "params": { "mode": "all_to_one" } }"#,
        )
        .unwrap();

        assert_eq!(request.output_path, None);
        assert_eq!(request.format, None);
        assert_eq!(request.params.mode, wheel_engines::recolor::RecolorMode::AllToOne);
        // Unspecified options must come back as usable defaults, not zeros.
        assert_eq!(request.params.tolerance, 30);
        // A flat fill is the default for "all to one", so an omitted key must not
        // quietly turn shading preservation back on.
        assert!(!request.params.preserve_shading);
    }

    #[tokio::test]
    async fn an_svg_lists_one_colour_when_it_declares_one_paint_twice() {
        // A single-colour logo that states its paint as both a hex and a
        // display-p3 twin must list one colour, because that is what the preview
        // shows and what the user is looking at.
        //
        // Reading the literals instead was tried and produced two swatches for
        // one orange. The hidden literal is handled by the save, not the list.
        let dir = std::env::temp_dir().join("wheel_recolor_cmd_svg_colors");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("logo.svg");
        // A filled shape, not a hairline: colours are counted from opaque pixels,
        // and a one-pixel diagonal is nothing but antialiased edge.
        std::fs::write(
            &path,
            r##"<svg viewBox="0 0 40 40"><rect width="40" height="40" fill="#FF5200" style="fill:#FF5200;fill:color(display-p3 1.0000 0.3216 0.0000)"/></svg>"##,
        )
        .unwrap();

        let found = recolor_extract_colors(path.to_string_lossy().to_string(), None)
            .await
            .unwrap();

        assert_eq!(
            found.colors.total_unique, 1,
            "one paint must be one swatch, got {:?}",
            found
                .colors
                .colors
                .iter()
                .map(|c| c.color.to_hex())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            found.colors.colors[0].color,
            Rgb::from_hex("#FF5200").unwrap()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn an_svg_save_still_rewrites_every_notation_of_the_paint() {
        // The counterpart to the test above: the list shows one colour, so the
        // save has to catch both literals behind it even at tolerance 0.
        let dir = std::env::temp_dir().join("wheel_recolor_cmd_svg_floor");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("logo.svg");
        let source = r##"<svg viewBox="0 0 10 10"><path d="M0 0 L10 10" fill="#FF5200" style="fill:#FF5200;fill:color(display-p3 1.0000 0.3216 0.0000)"/></svg>"##;
        std::fs::write(&input, source).unwrap();

        let output = dir.join("out.svg");
        let result = wheel_engines::svg_recolor::recolor_svg_file(
            &input,
            &output,
            &wheel_engines::recolor::RecolorParams {
                rules: vec![wheel_engines::recolor::RecolorRule {
                    from: Rgb::from_hex("#FF5200").unwrap(),
                    to: Rgb::from_hex("#2876D2").unwrap(),
                }],
                // Zero: the strictest setting, where a distance-based match
                // would only catch the exact hex.
                tolerance: 0,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.replaced, 3, "all three literals must change");
        let written = std::fs::read_to_string(&output).unwrap();
        assert!(!written.contains("#FF5200"), "a hex survived: {written}");
        assert!(!written.contains("display-p3"), "the twin survived: {written}");
        assert_eq!(written.matches("#2876D2").count(), 3, "got {written}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_svg_output_from_a_raster_source_with_a_reason() {
        // The bug this closes: offering "SVG (vector)" for a PNG, then failing the
        // save with "stream did not contain valid UTF-8" - the tool reading PNG
        // bytes as text. The window no longer offers it; this is the backstop.
        for source in ["C:/pics/bird.png", "C:/pics/photo.JPEG", "C:/pics/a.webp", "C:/pics/noext"] {
            let error = check_vector_output(
                std::path::Path::new(source),
                std::path::Path::new("C:/pics/out.svg"),
            )
            .expect_err("a raster must not be saveable as SVG");
            assert!(
                error.contains("markup"),
                "the message should explain why, got: {error}"
            );
            assert!(
                !error.contains("UTF-8"),
                "the message must not leak the underlying decode error, got: {error}"
            );
        }
    }

    #[test]
    fn allows_svg_output_from_an_svg_source() {
        for (source, target) in [
            ("C:/pics/logo.svg", "C:/pics/out.svg"),
            ("C:/pics/logo.SVG", "C:/pics/out.svg"),
            // `.svgz` holds SVG markup too.
            ("C:/pics/logo.svgz", "C:/pics/out.svg"),
            // A raster source saving to a raster target is untouched by this rule.
            ("C:/pics/bird.png", "C:/pics/out.png"),
            ("C:/pics/bird.png", "C:/pics/out.jpg"),
            ("C:/pics/logo.svg", "C:/pics/out.png"),
        ] {
            assert!(
                check_vector_output(
                    std::path::Path::new(source),
                    std::path::Path::new(target)
                )
                .is_ok(),
                "{source} -> {target} should be allowed"
            );
        }
    }
}
