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
    /// JPEG quality 1-100 (ignored for lossless formats)
    pub quality: u8,
}

pub fn decode_svg_bytes(bytes: &[u8]) -> Result<image::DynamicImage> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();

    let tree = resvg::usvg::Tree::from_data(bytes, &opt)
        .context("Failed to parse SVG data")?;

    let size = tree.size().to_int_size();
    let width = size.width().max(1);
    let height = size.height().max(1);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("Failed to allocate SVG pixmap of size {}x{}", width, height))?;

    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());

    let rgba = image::RgbaImage::from_raw(width, height, pixmap.take())
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
                let tmp_png = temp_dir.join(format!(
                    "decode_{}.png",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ));
                let status = std::process::Command::new(ffmpeg)
                    .arg("-y")
                    .arg("-i")
                    .arg(input)
                    .arg(&tmp_png)
                    .status();
                if let Ok(st) = status {
                    if st.success() {
                        let decoded = image::open(&tmp_png);
                        let _ = std::fs::remove_file(&tmp_png);
                        if let Ok(loaded) = decoded {
                            return Ok(loaded);
                        }
                    }
                }
                let _ = std::fs::remove_file(&tmp_png);
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

    let encode_result: Result<()> = (|| {
        match &params.output_format {
            OutputFormat::Jpeg => {
                // JPEG does not support transparency. If image has alpha, flatten onto white.
                let dynamic_rgb = if img.color().has_alpha() {
                    image::DynamicImage::ImageRgb8(flatten_to_rgb(&img, [255, 255, 255]))
                } else {
                    img
                };
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    std::fs::File::create(&tmp)
                        .with_context(|| format!("Cannot create {:?}", tmp))?,
                    params.quality,
                );
                dynamic_rgb.write_with_encoder(encoder)
                    .with_context(|| "JPEG encode failed")?;
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
                    let crf = (63 - ((params.quality as f32 / 100.0) * 50.0).round() as u32).clamp(10, 50);
                    let mut cmd = std::process::Command::new(ffmpeg);
                    cmd.arg("-y").arg("-i").arg(input);

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
                        .arg(crf.to_string())
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
            quality: 80,
        };

        let result = convert_image(&src_path, &params).expect("convert_image failed");
        assert_eq!(result, dst_path);
        assert!(dst_path.exists());
        assert!(std::fs::metadata(&dst_path).unwrap().len() > 0);

        let avif_params = ConvertParams {
            output_format: OutputFormat::Avif,
            output_path: avif_path.clone(),
            quality: 80,
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
            quality: 85,
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
            quality: 95,
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
            quality: 85,
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
            quality: 90,
        };
        let out_jpg = convert_image(&svg_path, &params_jpg).expect("convert svg to jpg failed");
        assert_eq!(out_jpg, jpg_path);
        assert!(jpg_path.exists());

        // Clean up
        let _ = std::fs::remove_file(svg_path);
        let _ = std::fs::remove_file(png_path);
        let _ = std::fs::remove_file(jpg_path);
    }
}
