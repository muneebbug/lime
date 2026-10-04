use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::ImageFormat;
use tracing::info;

/// Supported output formats for image conversion (M0/M1 subset)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
    Webp,
    Bmp,
    Tiff,
    Gif,
    Ico,
    Avif,
}

impl OutputFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "bmp" => Some(Self::Bmp),
            "tiff" | "tif" => Some(Self::Tiff),
            "gif" => Some(Self::Gif),
            "ico" => Some(Self::Ico),
            "avif" => Some(Self::Avif),
            _ => None,
        }
    }

    pub fn image_format(&self) -> Option<ImageFormat> {
        match self {
            Self::Png => Some(ImageFormat::Png),
            Self::Jpeg => Some(ImageFormat::Jpeg),
            Self::Webp => Some(ImageFormat::WebP),
            Self::Bmp => Some(ImageFormat::Bmp),
            Self::Tiff => Some(ImageFormat::Tiff),
            Self::Gif => Some(ImageFormat::Gif),
            Self::Ico => Some(ImageFormat::Ico),
            Self::Avif => Some(ImageFormat::Avif),
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Gif => "gif",
            Self::Ico => "ico",
            Self::Avif => "avif",
        }
    }
}

pub struct ConvertParams {
    pub output_format: OutputFormat,
    pub output_path: PathBuf,
    /// Carry EXIF/XMP/ICC from the source into the output where the target
    /// format supports it.
    pub preserve_metadata: bool,
}

/// JPEG has no lossless mode, so 100 is the least destructive setting available.
/// Every other target is encoded losslessly.
const JPEG_MAX_QUALITY: u8 = 100;

/// Metadata lifted off a source image, ready to be re-attached to an encoder.
#[derive(Debug, Default, Clone)]
pub struct SourceMetadata {
    pub exif: Option<Vec<u8>>,
    pub icc: Option<Vec<u8>>,
}

impl SourceMetadata {
    /// Read whatever metadata the source format exposes.
    ///
    /// Best-effort: a format that stores none, or a decoder that refuses to
    /// hand it over, simply yields empty fields rather than failing the
    /// conversion.
    pub fn read(path: &Path) -> Self {
        use image::{ImageDecoder, ImageReader};

        let Ok(reader) = ImageReader::open(path) else {
            return Self::default();
        };
        let Ok(reader) = reader.with_guessed_format() else {
            return Self::default();
        };
        let Ok(mut decoder) = reader.into_decoder() else {
            return Self::default();
        };

        Self {
            exif: decoder.exif_metadata().ok().flatten(),
            icc: decoder.icc_profile().ok().flatten(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.exif.is_none() && self.icc.is_none()
    }
}

/// Checks if a CSS property value uses an unsupported CSS Color 4 function
fn is_unsupported_color_fn(val: &str) -> bool {
    let v = val.trim().to_lowercase();
    v.starts_with("color(") || v.starts_with("oklab(") || v.starts_with("oklch(") || v.starts_with("lab(") || v.starts_with("lch(")
}

/// Convert CSS Color 4 `color(...)` function (e.g. `color(display-p3 1.0 0.3216 0.0)`) to `rgb(...)` or `rgba(...)`
fn convert_color_fn_to_rgb(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let lower = trimmed.to_lowercase();
    let inner = lower.strip_prefix("color(")?.strip_suffix(')')?.trim();

    let (colors_part, alpha_part) = if let Some((c, a)) = inner.split_once('/') {
        (c.trim(), Some(a.trim()))
    } else {
        (inner, None)
    };

    let mut tokens = colors_part.split_whitespace();
    let _space = tokens.next()?;
    let r_str = tokens.next()?;
    let g_str = tokens.next()?;
    let b_str = tokens.next()?;
    let extra_alpha_str = tokens.next();

    let parse_channel = |s: &str| -> Option<f32> {
        if let Some(pct) = s.strip_suffix('%') {
            pct.parse::<f32>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0))
        } else {
            let v = s.parse::<f32>().ok()?;
            if v > 1.0 {
                Some((v / 255.0).clamp(0.0, 1.0))
            } else {
                Some(v.clamp(0.0, 1.0))
            }
        }
    };

    let r_val = parse_channel(r_str)?;
    let g_val = parse_channel(g_str)?;
    let b_val = parse_channel(b_str)?;

    let alpha_val = if let Some(a_str) = alpha_part.or(extra_alpha_str) {
        parse_channel(a_str)
    } else {
        None
    };

    let r_u8 = (r_val * 255.0).round().clamp(0.0, 255.0) as u8;
    let g_u8 = (g_val * 255.0).round().clamp(0.0, 255.0) as u8;
    let b_u8 = (b_val * 255.0).round().clamp(0.0, 255.0) as u8;

    if let Some(a) = alpha_val {
        Some(format!("rgba({}, {}, {}, {:.3})", r_u8, g_u8, b_u8, a))
    } else {
        Some(format!("rgb({}, {}, {})", r_u8, g_u8, b_u8))
    }
}

/// Sanitizes CSS declarations within a `style="..."` attribute or CSS rule `{ ... }`.
///
/// Figma and other modern design tools export CSS fallback patterns like:
/// `fill:#FF5200;fill:color(display-p3 1.0000 0.3216 0.0000);fill-opacity:1;`
/// Standard CSS parsers keep the fallback (`#FF5200`) when `color(...)` is unsupported.
/// Resvg's parser however records the unsupported `color(...)`, fails at paint parse time,
/// and falls back to default solid black `#000000`.
///
/// This function drops the unsupported `color(...)` when a standard fallback is already present,
/// or converts standalone `color(...)` into standard `rgb(...)`.
fn sanitize_css_declarations(block: &str) -> String {
    let parts: Vec<&str> = block.split(';').collect();
    let mut standard_properties = std::collections::HashSet::new();

    for part in &parts {
        let trimmed = part.trim();
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_lowercase();
            let val = v.trim();
            if !val.is_empty() && !is_unsupported_color_fn(val) {
                standard_properties.insert(key);
            }
        }
    }

    let mut result_parts = Vec::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_lowercase();
            let val = v.trim();
            if is_unsupported_color_fn(val) {
                // If a standard fallback is already present for this property, drop the unsupported override!
                if standard_properties.contains(&key) {
                    continue;
                }
                // If no fallback was provided, convert color(...) into standard rgb(...)
                if let Some(converted) = convert_color_fn_to_rgb(val) {
                    result_parts.push(format!("{}: {}", k.trim(), converted));
                    continue;
                }
                continue;
            }
        }
        result_parts.push(trimmed.to_string());
    }

    if result_parts.is_empty() {
        String::new()
    } else {
        let mut joined = result_parts.join("; ");
        if block.trim_end().ends_with(';') {
            joined.push(';');
        }
        joined
    }
}

fn sanitize_style_tag_css(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    while let Some(brace_start) = css[i..].find('{') {
        let abs_start = i + brace_start;
        out.push_str(&css[i..=abs_start]);
        let decl_start = abs_start + 1;
        if let Some(brace_end) = css[decl_start..].find('}') {
            let abs_end = decl_start + brace_end;
            let decls = &css[decl_start..abs_end];
            out.push_str(&sanitize_css_declarations(decls));
            out.push('}');
            i = abs_end + 1;
        } else {
            out.push_str(&css[decl_start..]);
            return out;
        }
    }
    out.push_str(&css[i..]);
    out
}

pub fn sanitize_svg_string(svg: &str) -> String {
    if !svg.contains("color(") && !svg.contains("oklab(") && !svg.contains("oklch(") && !svg.contains("lab(") && !svg.contains("lch(") {
        return svg.to_string();
    }

    let mut output = String::with_capacity(svg.len());
    let chars: Vec<(usize, char)> = svg.char_indices().collect();
    let num_chars = chars.len();

    let mut char_idx = 0;
    while char_idx < num_chars {
        let (byte_pos, _) = chars[char_idx];
        let slice = &svg[byte_pos..];

        // Check for <style...> ... </style>
        if slice.len() >= 6 && slice[..6].eq_ignore_ascii_case("<style") {
            let next_ch = slice[6..].chars().next();
            if next_ch == Some('>') || next_ch.map(|c| c.is_whitespace()).unwrap_or(false) {
                if let Some(open_end) = slice.find('>') {
                    let open_tag_bytes = &slice[..open_end + 1];
                    let after_open = &slice[open_end + 1..];
                    if let Some(close_idx) = after_open.to_ascii_lowercase().find("</style>") {
                        output.push_str(open_tag_bytes);
                        let css_block = &after_open[..close_idx];
                        output.push_str(&sanitize_style_tag_css(css_block));
                        let total_consumed = open_end + 1 + close_idx;
                        while char_idx < num_chars && chars[char_idx].0 < byte_pos + total_consumed {
                            char_idx += 1;
                        }
                        continue;
                    }
                }
            }
        }

        // Check for style=" or style='
        if slice.len() >= 7 && slice[..5].eq_ignore_ascii_case("style") {
            let after_style = &slice[5..];
            let trimmed_start = after_style.trim_start();
            if let Some(after_eq) = trimmed_start.strip_prefix('=') {
                let trimmed_after_eq = after_eq.trim_start();
                if trimmed_after_eq.starts_with('"') || trimmed_after_eq.starts_with('\'') {
                    let quote = trimmed_after_eq.chars().next().unwrap();
                    let val_content = &trimmed_after_eq[1..];
                    if let Some(quote_end) = val_content.find(quote) {
                        let content = &val_content[..quote_end];
                        let sanitized_style = sanitize_css_declarations(content);

                        let prefix_len = slice.len() - val_content.len();
                        output.push_str(&slice[..prefix_len]);
                        output.push_str(&sanitized_style);
                        output.push(quote);

                        let total_consumed = prefix_len + quote_end + 1;
                        while char_idx < num_chars && chars[char_idx].0 < byte_pos + total_consumed {
                            char_idx += 1;
                        }
                        continue;
                    }
                }
            }
        }

        // Check for attribute with color(...) e.g. ="color(...) or ='color(...)
        if slice.len() >= 8 && (slice.starts_with("=\"color(") || slice.starts_with("='color(")) {
            let quote = slice.chars().nth(1).unwrap();
            let val_content = &slice[2..];
            if let Some(quote_end) = val_content.find(quote) {
                let content = &val_content[..quote_end];
                let converted = convert_color_fn_to_rgb(content).unwrap_or_else(|| content.to_string());
                output.push('=');
                output.push(quote);
                output.push_str(&converted);
                output.push(quote);

                let total_consumed = 2 + quote_end + 1;
                while char_idx < num_chars && chars[char_idx].0 < byte_pos + total_consumed {
                    char_idx += 1;
                }
                continue;
            }
        }

        output.push(chars[char_idx].1);
        char_idx += 1;
    }

    output
}

pub fn decode_svg_bytes(bytes: &[u8]) -> Result<image::DynamicImage> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();

    // Sanitize SVG if it contains CSS Color 4 functions (like display-p3 from Figma)
    let sanitized_bytes: std::borrow::Cow<[u8]> = if let Ok(s) = std::str::from_utf8(bytes) {
        if s.contains("color(") || s.contains("oklab(") || s.contains("oklch(") || s.contains("lab(") || s.contains("lch(") {
            let clean = sanitize_svg_string(s);
            std::borrow::Cow::Owned(clean.into_bytes())
        } else {
            std::borrow::Cow::Borrowed(bytes)
        }
    } else {
        std::borrow::Cow::Borrowed(bytes)
    };

    let tree = resvg::usvg::Tree::from_data(&sanitized_bytes, &opt)
        .context("Failed to parse SVG data")?;

    let size = tree.size().to_int_size();
    let width = size.width().max(1);
    let height = size.height().max(1);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("Failed to allocate SVG pixmap of size {}x{}", width, height))?;

    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // tiny_skia renders pixels with premultiplied alpha (PremultipliedColorU8).
    // The image crate's RgbaImage expects straight (unpremultiplied) RGBA.
    // Un-premultiply each pixel so semi-transparent antialiased edges do not become
    // dark/black outlines or lose color accuracy.
    let mut raw = pixmap.take();
    for pixel in raw.chunks_exact_mut(4) {
        let a = pixel[3];
        if a > 0 && a < 255 {
            let a32 = a as u32;
            pixel[0] = ((pixel[0] as u32 * 255 + a32 / 2) / a32).min(255) as u8;
            pixel[1] = ((pixel[1] as u32 * 255 + a32 / 2) / a32).min(255) as u8;
            pixel[2] = ((pixel[2] as u32 * 255 + a32 / 2) / a32).min(255) as u8;
        }
    }

    let rgba = image::RgbaImage::from_raw(width, height, raw)
        .ok_or_else(|| anyhow::anyhow!("Failed to construct RgbaImage from rendered SVG pixmap"))?;

    Ok(image::DynamicImage::ImageRgba8(rgba))
}

pub fn load_svg(path: &Path) -> Result<image::DynamicImage> {
    let bytes = std::fs::read(path).with_context(|| format!("Failed to read SVG file at {:?}", path))?;
    decode_svg_bytes(&bytes)
}

fn is_svg_bytes(bytes: &[u8]) -> bool {
    let prefix_len = bytes.len().min(512);
    if let Ok(prefix) = std::str::from_utf8(&bytes[..prefix_len]) {
        let lower = prefix.to_lowercase();
        lower.contains("<svg")
    } else {
        false
    }
}

/// Load an image from path. Supports all standard raster formats, SVG vector files, and FFmpeg fallback.
pub fn load_image(input: &Path) -> Result<image::DynamicImage> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "svg" || ext == "svgz" {
        return load_svg(input);
    }

    match image::open(input) {
        Ok(img) => Ok(img),
        Err(orig_err) => {
            // Check if file is actually an SVG despite missing or mismatched extension
            if let Ok(content) = std::fs::read(input) {
                if is_svg_bytes(&content) {
                    if let Ok(svg_img) = decode_svg_bytes(&content) {
                        return Ok(svg_img);
                    }
                }
            }

            // If image::open fails on unsupported formats, attempt to decode with FFmpeg
            if let Some(ffmpeg) = crate::media::find_ffmpeg_path() {
                let temp_dir = std::env::temp_dir().join("wheel_temp");
                let _ = std::fs::create_dir_all(&temp_dir);
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let merged_png = temp_dir.join(format!("decode_{stamp}_merged.png"));
                let plain_png = temp_dir.join(format!("decode_{stamp}.png"));

                // Formats like AVIF store transparency as a second, grayscale
                // stream. FFmpeg's automatic stream selection picks only the
                // colour stream, so the alpha is dropped and transparent
                // padding silently becomes opaque — which makes Trim a no-op.
                // Merging explicitly keeps it. This writes nothing when the
                // source has no second stream, so file existence is the signal.
                let merged = crate::media::no_window_command(&ffmpeg)
                    .arg("-y")
                    .arg("-i")
                    .arg(input)
                    .arg("-filter_complex")
                    .arg("[0:v:0][0:v:1]alphamerge")
                    .arg("-frames:v")
                    .arg("1")
                    .arg(&merged_png)
                    .output()
                    .is_ok();

                let chosen = if merged && std::fs::metadata(&merged_png).is_ok_and(|m| m.len() > 0)
                {
                    merged_png
                } else {
                    let _ = std::fs::remove_file(&merged_png);
                    // `-frames:v 1` is required: without it the image2 muxer
                    // refuses to write a multi-frame source to one filename.
                    let ok = crate::media::no_window_command(&ffmpeg)
                        .arg("-y")
                        .arg("-i")
                        .arg(input)
                        .arg("-frames:v")
                        .arg("1")
                        .arg(&plain_png)
                        .output()
                        .is_ok();
                    if ok && std::fs::metadata(&plain_png).is_ok_and(|m| m.len() > 0) {
                        plain_png
                    } else {
                        let _ = std::fs::remove_file(&plain_png);
                        return Err(orig_err)
                            .with_context(|| format!("Failed to open image {:?}", input));
                    }
                };

                let decoded = image::open(&chosen);
                let _ = std::fs::remove_file(&chosen);
                if let Ok(loaded) = decoded {
                    return Ok(loaded);
                }
            }
            Err(orig_err).with_context(|| format!("Failed to open image {:?}", input))
        }
    }
}

/// Convert an image file to another format.
/// This runs synchronously — call from `tokio::task::spawn_blocking`.
pub fn convert_image(input: &Path, params: &ConvertParams) -> Result<PathBuf> {
    let img = load_image(input)?;
    convert_image_loaded(img, input, params)
}

/// Flatten an image with an alpha channel onto a solid background color (typically pure white [255, 255, 255]).
///
/// Formats like JPEG do not support transparency. When a transparent PNG is naively converted to RGB,
/// the alpha channel is simply discarded, exposing whatever RGB values happen to be in the transparent
/// pixels (often (0,0,0) black or (255,255,255) white blocks).
///
/// Alpha compositing blends transparent and semi-transparent pixels smoothly against the background:
/// `result = foreground * alpha + background * (1 - alpha)`
pub fn flatten_to_rgb(img: &image::DynamicImage, bg_color: [u8; 3]) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(width, height);

    let bg_r = bg_color[0] as f32 / 255.0;
    let bg_g = bg_color[1] as f32 / 255.0;
    let bg_b = bg_color[2] as f32 / 255.0;

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = pixel[3] as f32 / 255.0;
        if alpha >= 0.999 {
            rgb.put_pixel(x, y, image::Rgb([pixel[0], pixel[1], pixel[2]]));
        } else if alpha <= 0.001 {
            rgb.put_pixel(x, y, image::Rgb(bg_color));
        } else {
            let fg_r = pixel[0] as f32 / 255.0;
            let fg_g = pixel[1] as f32 / 255.0;
            let fg_b = pixel[2] as f32 / 255.0;

            let out_r = ((fg_r * alpha + bg_r * (1.0 - alpha)) * 255.0).round() as u8;
            let out_g = ((fg_g * alpha + bg_g * (1.0 - alpha)) * 255.0).round() as u8;
            let out_b = ((fg_b * alpha + bg_b * (1.0 - alpha)) * 255.0).round() as u8;

            rgb.put_pixel(x, y, image::Rgb([out_r, out_g, out_b]));
        }
    }

    rgb
}

/// Hand collected metadata to an encoder.
///
/// Formats that cannot store a given item reject it, which is not an error —
/// the pixels still encode correctly.
fn apply_metadata(encoder: &mut impl image::ImageEncoder, meta: &SourceMetadata) {
    if let Some(exif) = &meta.exif {
        let _ = encoder.set_exif_metadata(exif.clone());
    }
    if let Some(icc) = &meta.icc {
        let _ = encoder.set_icc_profile(icc.clone());
    }
}

fn convert_image_loaded(img: image::DynamicImage, input: &Path, params: &ConvertParams) -> Result<PathBuf> {
    info!(
        "Converting {:?} -> {:?} ({:?})",
        input,
        params.output_path,
        params.output_format
    );

    // Create temporary file in the OS temp directory with its proper format extension
    // so FFmpeg and other encoders recognize the container format immediately.
    let temp_dir = std::env::temp_dir().join("wheel_temp");
    let _ = std::fs::create_dir_all(&temp_dir);
    let tmp = temp_dir.join(format!(
        "conv_{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        params.output_format.extension()
    ));

    let meta = if params.preserve_metadata {
        SourceMetadata::read(input)
    } else {
        SourceMetadata::default()
    };

    let encode_result: Result<()> = (|| {
        match &params.output_format {
            OutputFormat::Jpeg => {
                // JPEG does not support transparency. If image has alpha, flatten onto white.
                let dynamic_rgb = if img.color().has_alpha() {
                    image::DynamicImage::ImageRgb8(flatten_to_rgb(&img, [255, 255, 255]))
                } else {
                    img
                };
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    std::fs::File::create(&tmp)
                        .with_context(|| format!("Cannot create {:?}", tmp))?,
                    JPEG_MAX_QUALITY,
                );
                apply_metadata(&mut encoder, &meta);
                dynamic_rgb.write_with_encoder(encoder)
                    .with_context(|| "JPEG encode failed")?;
            }
            OutputFormat::Png => {
                let mut encoder = image::codecs::png::PngEncoder::new(
                    std::fs::File::create(&tmp)
                        .with_context(|| format!("Cannot create {:?}", tmp))?,
                );
                apply_metadata(&mut encoder, &meta);
                img.write_with_encoder(encoder)
                    .with_context(|| "PNG encode failed")?;
            }
            OutputFormat::Webp => {
                let mut encoder = image::codecs::webp::WebPEncoder::new_lossless(
                    std::fs::File::create(&tmp)
                        .with_context(|| format!("Cannot create {:?}", tmp))?,
                );
                apply_metadata(&mut encoder, &meta);
                img.write_with_encoder(encoder)
                    .with_context(|| "WebP encode failed")?;
            }
            OutputFormat::Bmp => {
                // BMP typically does not support transparency. Flatten onto white if alpha present.
                let dynamic_rgb = if img.color().has_alpha() {
                    image::DynamicImage::ImageRgb8(flatten_to_rgb(&img, [255, 255, 255]))
                } else {
                    img
                };
                dynamic_rgb.save_with_format(&tmp, ImageFormat::Bmp)
                    .with_context(|| "BMP encode failed")?;
            }
            OutputFormat::Ico => {
                let ico_img = if img.width() > 256 || img.height() > 256 {
                    img.resize(256, 256, image::imageops::FilterType::Lanczos3)
                } else {
                    img
                };
                ico_img.save_with_format(&tmp, ImageFormat::Ico)
                    .with_context(|| "ICO encode failed")?;
            }
            OutputFormat::Avif => {
                // Official FFmpeg documentation for AVIF handling:
                // For transparent images, FFmpeg maps the color stream (0:0) and extracts the alpha
                // channel (0:1) with the `alphaextract` filter, encoding as a standard MIAF still picture
                // with libaom-av1:
                // `ffmpeg -i input.png -map 0 -map 0 -filter:v:1 alphaextract -frames:v 1 -c:v libaom-av1 -still-picture 1 -crf <crf> output.avif`
                //
                // For opaque images:
                // `ffmpeg -i input.png -frames:v 1 -c:v libaom-av1 -still-picture 1 -crf <crf> output.avif`
                let mut ffmpeg_encoded = false;
                if let Some(ffmpeg) = crate::media::find_ffmpeg_path() {
                    // Always hand FFmpeg a PNG of the already-decoded image rather than the
                    // original file. FFmpeg has no SVG decoder (and may not understand other
                    // inputs `load_image` handled via fallbacks), which previously made the
                    // FFmpeg encode fail and fall through to the lossy-alpha fallback below.
                    let tmp_src = temp_dir.join(format!(
                        "avif_src_{}.png",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos()
                    ));
                    let wrote_src = img.save_with_format(&tmp_src, ImageFormat::Png).is_ok();

                    if wrote_src {
                        // `-crf 0` selects libaom's lossless mode, so the AVIF
                        // round-trips bit-exact. A lossy CRF smears the alpha
                        // plane at edges, which made Trim measure the wrong
                        // bounding box on files this app had just written.
                        let mut cmd = crate::media::no_window_command(ffmpeg);
                        cmd.arg("-y").arg("-i").arg(&tmp_src);

                        if img.color().has_alpha() {
                            cmd.arg("-map")
                                .arg("0")
                                .arg("-map")
                                .arg("0")
                                .arg("-filter:v:1")
                                .arg("alphaextract");
                        }

                        cmd.arg("-frames:v")
                            .arg("1")
                            .arg("-c:v")
                            .arg("libaom-av1")
                            .arg("-crf")
                            .arg("0")
                            .arg("-cpu-used")
                            .arg("8")
                            .arg("-row-mt")
                            .arg("1")
                            .arg("-still-picture")
                            .arg("1")
                            .arg("-f")
                            .arg("avif")
                            .arg(&tmp);

                        #[cfg(windows)]
                        {
                            use std::os::windows::process::CommandExt;
                            cmd.creation_flags(0x08000000);
                        }

                        match cmd.output() {
                            Ok(out) if out.status.success() => {
                                ffmpeg_encoded = true;
                            }
                            Ok(out) => {
                                tracing::warn!("FFmpeg AVIF encode returned error: {}", String::from_utf8_lossy(&out.stderr));
                            }
                            Err(e) => {
                                tracing::warn!("Failed to execute FFmpeg for AVIF: {:?}", e);
                            }
                        }
                        let _ = std::fs::remove_file(&tmp_src);
                    }
                }

                if !ffmpeg_encoded {
                    // Pre-built fallback using the standard `image` crate AVIF encoder
                    img.save_with_format(&tmp, ImageFormat::Avif)
                        .with_context(|| "AVIF encode failed")?;
                }
            }
            fmt => {
                if let Some(image_fmt) = fmt.image_format() {
                    img.save_with_format(&tmp, image_fmt)
                        .with_context(|| format!("Encode to {:?} failed", fmt))?;
                } else {
                    anyhow::bail!("Unsupported image format: {:?}", fmt);
                }
            }
        }
        Ok(())
    })();

    if let Err(e) = encode_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    if let Some(parent) = params.output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(_rename_err) = std::fs::rename(&tmp, &params.output_path) {
        std::fs::copy(&tmp, &params.output_path)
            .with_context(|| format!("Failed to save output to {:?}", params.output_path))?;
        let _ = std::fs::remove_file(&tmp);
    }

    info!("Conversion complete: {:?}", params.output_path);
    Ok(params.output_path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_from_ext() {
        assert_eq!(OutputFormat::from_extension("png"), Some(OutputFormat::Png));
        assert_eq!(OutputFormat::from_extension("JPG"), Some(OutputFormat::Jpeg));
        assert_eq!(OutputFormat::from_extension("jpeg"), Some(OutputFormat::Jpeg));
        assert_eq!(OutputFormat::from_extension("webp"), Some(OutputFormat::Webp));
        assert_eq!(OutputFormat::from_extension("bmp"), Some(OutputFormat::Bmp));
        assert_eq!(OutputFormat::from_extension("tiff"), Some(OutputFormat::Tiff));
        assert_eq!(OutputFormat::from_extension("gif"), Some(OutputFormat::Gif));
        assert_eq!(OutputFormat::from_extension("ico"), Some(OutputFormat::Ico));
        assert_eq!(OutputFormat::from_extension("avif"), Some(OutputFormat::Avif));
        assert_eq!(OutputFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_convert_image_roundtrip() {
        use image::{RgbaImage, Rgba};
        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);

        let src_path = dir.join("test_input.png");
        let dst_path = dir.join("test_output.webp");
        let avif_path = dir.join("test_output.avif");

        // Create a 16x16 test image
        let mut img = RgbaImage::new(16, 16);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([255, 128, 0, 255]);
        }
        img.save(&src_path).expect("failed to save test image");

        let params = ConvertParams {
            output_format: OutputFormat::Webp,
            output_path: dst_path.clone(),
            preserve_metadata: true,
        };

        let result = convert_image(&src_path, &params).expect("convert_image failed");
        assert_eq!(result, dst_path);
        assert!(dst_path.exists());
        assert!(std::fs::metadata(&dst_path).unwrap().len() > 0);

        let avif_params = ConvertParams {
            output_format: OutputFormat::Avif,
            output_path: avif_path.clone(),
            preserve_metadata: true,
        };
        let avif_result = convert_image(&src_path, &avif_params).expect("convert to avif failed");
        assert_eq!(avif_result, avif_path);
        assert!(avif_path.exists());
        assert!(std::fs::metadata(&avif_path).unwrap().len() > 0);

        // Clean up
        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(dst_path);
        let _ = std::fs::remove_file(avif_path);
    }

    #[test]
    fn test_avif_alpha_preservation() {
        use image::{RgbaImage, Rgba};
        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);

        let src_path = dir.join("test_avif_alpha_input.png");
        let dst_path = dir.join("test_avif_alpha_output.avif");

        // Create an image with transparent background, AI noise dust (alpha=2), and solid object
        let mut img = RgbaImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                if x >= 4 && x < 12 && y >= 4 && y < 12 {
                    img.put_pixel(x, y, Rgba([200, 50, 50, 255])); // opaque red box
                } else if x == 0 && y == 0 {
                    img.put_pixel(x, y, Rgba([30, 80, 20, 3])); // low-alpha dust (should be cleared)
                } else {
                    img.put_pixel(x, y, Rgba([10, 20, 30, 0])); // transparent with dirty unmasked RGB
                }
            }
        }
        img.save(&src_path).expect("failed to save input image");

        let params = ConvertParams {
            output_format: OutputFormat::Avif,
            output_path: dst_path.clone(),
        preserve_metadata: true,
        };

        let result = convert_image(&src_path, &params).expect("convert to avif failed");
        assert_eq!(result, dst_path);
        assert!(dst_path.exists());
        assert!(std::fs::metadata(&dst_path).unwrap().len() > 0);

        // Clean up
        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(dst_path);
    }

    #[test]
    fn test_svg_to_avif_converts_via_decoded_image() {
        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);

        let src_path = dir.join("test_svg_to_avif.svg");
        let dst_path = dir.join("test_svg_to_avif.avif");
        std::fs::write(
            &src_path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><circle cx="32" cy="32" r="24" fill="#ff4000"/><circle cx="32" cy="32" r="6" fill="#111111"/></svg>"##,
        )
        .unwrap();

        let params = ConvertParams {
            output_format: OutputFormat::Avif,
            output_path: dst_path.clone(),
        preserve_metadata: true,
        };
        let result = convert_image(&src_path, &params).expect("svg to avif failed");
        assert_eq!(result, dst_path);
        assert!(std::fs::metadata(&dst_path).unwrap().len() > 0);

        if std::env::var_os("WHEEL_KEEP_TEST_OUTPUT").is_none() {
            let _ = std::fs::remove_file(src_path);
            let _ = std::fs::remove_file(dst_path);
        }
    }

    #[test]
    fn test_convert_transparent_png_to_jpeg_composites_on_white() {
        use image::{RgbaImage, Rgba};
        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);

        let src_path = dir.join("test_alpha_input.png");
        let dst_path = dir.join("test_alpha_output.jpg");

        // 4x4 image:
        // (0,0): red opaque
        // (0,1): transparent with RGB=0,0,0, alpha=0 (common in transparent PNGs)
        // (1,0): transparent with RGB=255,255,255, alpha=0 (unmultiplied white artifact)
        // (1,1): 50% transparent red
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0]));
        img.put_pixel(0, 1, Rgba([255, 255, 255, 0]));
        img.put_pixel(1, 1, Rgba([255, 0, 0, 128]));
        img.save(&src_path).expect("failed to save alpha test image");

        let params = ConvertParams {
            output_format: OutputFormat::Jpeg,
            output_path: dst_path.clone(),
            preserve_metadata: true,
        };

        let result = convert_image(&src_path, &params).expect("convert_image failed");
        assert_eq!(result, dst_path);
        assert!(dst_path.exists());

        // Read back the JPEG and verify pixel colors
        let decoded = image::open(&dst_path).expect("failed to open output JPEG").to_rgb8();
        
        // (0,0) was pure opaque red -> still red (~255, ~0, ~0)
        let p_opaque = decoded.get_pixel(0, 0);
        assert!(p_opaque[0] > 240 && p_opaque[1] < 15 && p_opaque[2] < 15);

        // (1,0) was transparent with (0,0,0,0) -> must be WHITE, NOT BLACK!
        let p_trans_black = decoded.get_pixel(1, 0);
        assert!(p_trans_black[0] > 240 && p_trans_black[1] > 240 && p_trans_black[2] > 240,
            "Expected transparent pixel to composite onto white, got {:?}", p_trans_black);

        // (0,1) was transparent with (255,255,255,0) -> must also be WHITE!
        let p_trans_white = decoded.get_pixel(0, 1);
        assert!(p_trans_white[0] > 240 && p_trans_white[1] > 240 && p_trans_white[2] > 240,
            "Expected transparent pixel to composite onto white, got {:?}", p_trans_white);

        // Clean up
        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(dst_path);
    }

    #[test]
    fn test_convert_svg_to_png_and_jpeg() {
        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);

        let svg_path = dir.join("test_shape.svg");
        let png_path = dir.join("test_shape.png");
        let jpg_path = dir.join("test_shape.jpg");

        let svg_content = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
            <rect width="100" height="100" fill="blue" />
            <circle cx="50" cy="50" r="30" fill="red" />
        </svg>"#;
        std::fs::write(&svg_path, svg_content).expect("failed to write test svg");

        // Convert SVG to PNG
        let params_png = ConvertParams {
            output_format: OutputFormat::Png,
            output_path: png_path.clone(),
        preserve_metadata: true,
        };
        let out_png = convert_image(&svg_path, &params_png).expect("convert svg to png failed");
        assert_eq!(out_png, png_path);
        assert!(png_path.exists());
        let png_img = image::open(&png_path).expect("failed to open generated png");
        assert_eq!(png_img.width(), 100);
        assert_eq!(png_img.height(), 100);

        // Convert SVG to JPEG
        let params_jpg = ConvertParams {
            output_format: OutputFormat::Jpeg,
            output_path: jpg_path.clone(),
            preserve_metadata: true,
        };
        let out_jpg = convert_image(&svg_path, &params_jpg).expect("convert svg to jpg failed");
        assert_eq!(out_jpg, jpg_path);
        assert!(jpg_path.exists());

        // Clean up
        let _ = std::fs::remove_file(svg_path);
        let _ = std::fs::remove_file(png_path);
        let _ = std::fs::remove_file(jpg_path);
    }

    #[test]
    fn test_convert_svg_display_p3_color() {
        // Test with the exact user SVG snippet containing Display-P3 progressive enhancement from Figma
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="353" height="353" viewBox="0 0 353 353" fill="none">
<path d="M173.328 0.0576346C174.571 0.0111346 175.813 -0.00736574 177.056 0.00263426C187.428 0.124884 197.588 0.972888 207.798 2.86414C214.823 4.16514 222.453 5.96313 227.563 11.3794C237.791 22.2204 229.836 37.8004 225.076 49.3156L186.11 141.971C182.7 150.078 171.239 150.152 167.725 142.089L127.441 49.6579C120.362 32.5411 111.38 12.1726 137.043 4.51938C148.343 1.14963 161.574 0.440135 173.328 0.0576346Z" fill="#FF5200" style="fill:#FF5200;fill:color(display-p3 1.0000 0.3216 0.0000);fill-opacity:1;"/>
</svg>"##;
        let img = decode_svg_bytes(svg.as_bytes()).expect("SVG decode failed");
        let rgba = img.to_rgba8();

        // Sample an interior pixel in the path (e.g. x=175, y=50)
        let interior_pixel = rgba.get_pixel(175, 50);
        println!("Interior pixel: {:?}", interior_pixel);
        assert_eq!(interior_pixel[3], 255, "Interior pixel should be fully opaque");
        assert_eq!(interior_pixel[0], 255, "Red channel must be 255 (#FF5200)");
        assert_eq!(interior_pixel[1], 82, "Green channel must be 82 (#FF5200)");
        assert_eq!(interior_pixel[2], 0, "Blue channel must be 0 (#FF5200)");

        // Also test standalone display-p3 without fallback in style or attribute
        let svg_no_fallback = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
<rect width="100" height="100" style="fill:color(display-p3 1.0000 0.3216 0.0000);"/>
</svg>"##;
        let img2 = decode_svg_bytes(svg_no_fallback.as_bytes()).expect("SVG decode without fallback failed");
        let rgba2 = img2.to_rgba8();
        let p2 = rgba2.get_pixel(50, 50);
        println!("No fallback pixel: {:?}", p2);
        assert_eq!(p2[0], 255, "Red channel must be 255");
        assert_eq!(p2[1], 82, "Green channel must be 82");
        assert_eq!(p2[2], 0, "Blue channel must be 0");
    }

    /// Build a real EXIF APP1 segment: little-endian TIFF, one Artist tag.
    fn exif_app1(artist: &str) -> Vec<u8> {
        let mut tiff: Vec<u8> = Vec::new();
        tiff.extend_from_slice(b"Exif\0\0");
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&0x002Au16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes());
        let mut value = artist.as_bytes().to_vec();
        value.push(0);
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x010Fu16.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        tiff.extend_from_slice(&(value.len() as u32).to_le_bytes());
        tiff.extend_from_slice(&32u32.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(tiff.len(), 32);
        tiff.extend_from_slice(&value);

        let mut seg = vec![0xFF, 0xE1];
        seg.extend_from_slice(&((tiff.len() + 2) as u16).to_be_bytes());
        seg.extend_from_slice(&tiff);
        seg
    }

    /// Inject an APP1 EXIF segment straight after a JPEG's SOI marker.
    fn jpeg_with_exif(src: &[u8], artist: &str) -> Vec<u8> {
        assert_eq!(&src[0..2], &[0xFF, 0xD8], "not a JPEG");
        let mut out = Vec::new();
        out.extend_from_slice(&src[0..2]);
        out.extend_from_slice(&exif_app1(artist));
        out.extend_from_slice(&src[2..]);
        out
    }

    fn has_exif(path: &Path) -> bool {
        SourceMetadata::read(path).exif.is_some()
    }

    #[test]
    fn test_preserve_metadata_controls_exif_for_images() {
        let Some(ffmpeg) = crate::media::find_ffmpeg_path() else {
            return;
        };

        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);
        let base = dir.join("exif_base.jpg");
        let tagged = dir.join("exif_tagged.jpg");

        let made = crate::media::no_window_command(&ffmpeg)
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("testsrc=size=64x48:rate=1:duration=1")
            .arg("-frames:v")
            .arg("1")
            .arg(&base)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !made {
            return;
        }

        let raw = std::fs::read(&base).expect("base jpeg");
        std::fs::write(&tagged, jpeg_with_exif(&raw, "WHEELER")).unwrap();

        // Without a real EXIF segment on the input, every assertion below would pass
        // for the wrong reason.
        assert!(has_exif(&tagged), "fixture carries no EXIF");

        for (fmt, ext) in [
            (OutputFormat::Png, "png"),
            (OutputFormat::Jpeg, "jpg"),
            (OutputFormat::Webp, "webp"),
        ] {
            for (preserve, expect) in [(true, true), (false, false)] {
                let out = dir.join(format!("exif_{}_{}.{ext}", preserve, ext));
                let _ = std::fs::remove_file(&out);
                convert_image(
                    &tagged,
                    &ConvertParams {
                        output_format: fmt.clone(),
                        output_path: out.clone(),
                        preserve_metadata: preserve,
                    },
                )
                .expect("convert failed");

                assert_eq!(
                    has_exif(&out),
                    expect,
                    "{ext}: preserve_metadata={preserve} produced the wrong result"
                );
                let _ = std::fs::remove_file(&out);
            }
        }

        let _ = std::fs::remove_file(&base);
        let _ = std::fs::remove_file(&tagged);
    }

    #[test]
    fn test_source_metadata_reads_nothing_from_svg() {
        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);
        let svg = dir.join("meta_none.svg");
        std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"##).unwrap();
        let meta = SourceMetadata::read(&svg);
        assert!(meta.is_empty(), "SVG should not yield metadata");
        let _ = std::fs::remove_file(&svg);
    }
}