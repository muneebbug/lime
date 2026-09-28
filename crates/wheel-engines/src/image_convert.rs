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
    Heic,
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
            "heic" | "heif" => Some(Self::Heic),
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
            Self::Heic => None,
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
            Self::Heic => "heic",
        }
    }
}

pub struct ConvertParams {
    pub output_format: OutputFormat,
    pub output_path: PathBuf,
    /// JPEG quality 1-100 (ignored for lossless formats)
    pub quality: u8,
}

/// Convert an image file to another format.
/// This runs synchronously — call from `tokio::task::spawn_blocking`.
pub fn convert_image(input: &Path, params: &ConvertParams) -> Result<PathBuf> {
    let img = match image::open(input) {
        Ok(img) => img,
        Err(orig_err) => {
            // If image::open fails (e.g. HEIC/HEIF), attempt to decode with FFmpeg
            if let Some(ffmpeg) = crate::media::find_ffmpeg_path() {
                let tmp_png = params.output_path.with_extension("wheel_decoded_tmp.png");
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
                            return convert_image_loaded(loaded, input, params);
                        }
                    }
                }
                let _ = std::fs::remove_file(&tmp_png);
            }
            return Err(orig_err).with_context(|| format!("Failed to open {:?}", input));
        }
    };

    convert_image_loaded(img, input, params)
}

fn convert_image_loaded(img: image::DynamicImage, input: &Path, params: &ConvertParams) -> Result<PathBuf> {
    info!(
        "Converting {:?} -> {:?} ({:?})",
        input,
        params.output_path,
        params.output_format
    );

    // Write to a temp file first, then atomic rename
    let tmp = params.output_path.with_extension(
        format!("{}.tmp", params.output_format.extension())
    );

    match &params.output_format {
        OutputFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::fs::File::create(&tmp)
                    .with_context(|| format!("Cannot create {:?}", tmp))?,
                params.quality,
            );
            img.write_with_encoder(encoder)
                .with_context(|| "JPEG encode failed")?;
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
            // Try image crate built-in AVIF encoder first, fallback to FFmpeg
            let res = img.save_with_format(&tmp, ImageFormat::Avif);
            if let Err(e) = res {
                tracing::warn!("Image crate AVIF encode failed ({:?}), trying FFmpeg fallback", e);
                let ffmpeg = crate::media::find_ffmpeg_path().ok_or_else(|| {
                    anyhow::anyhow!("AVIF encoding failed and FFmpeg not found: {}", e)
                })?;
                let status = std::process::Command::new(ffmpeg)
                    .arg("-y")
                    .arg("-i")
                    .arg(input)
                    .arg("-c:v")
                    .arg("libaom-av1")
                    .arg("-crf")
                    .arg("28")
                    .arg(&tmp)
                    .status()
                    .with_context(|| "Failed to execute FFmpeg for AVIF conversion")?;
                if !status.success() {
                    anyhow::bail!("FFmpeg AVIF conversion exited with code {:?}", status.code());
                }
            }
        }
        OutputFormat::Heic => {
            convert_to_heic(input, &tmp, params.quality)?;
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

    std::fs::rename(&tmp, &params.output_path)
        .with_context(|| format!("Atomic rename {:?} -> {:?} failed", tmp, params.output_path))?;

    info!("Conversion complete: {:?}", params.output_path);
    Ok(params.output_path.clone())
}

pub fn convert_to_heic(input: &Path, output: &Path, quality: u8) -> Result<()> {
    // 1. Try heif-enc if installed (e.g. %LOCALAPPDATA%\Wheel\bin\heif-enc.exe or in PATH)
    if let Some(tool) = find_heif_enc_path() {
        let status = std::process::Command::new(tool)
            .arg(input)
            .arg("-q")
            .arg(quality.to_string())
            .arg("-o")
            .arg(output)
            .status()
            .with_context(|| "Failed to execute heif-enc")?;
        if status.success() {
            return Ok(());
        }
    }

    // 2. Try ImageMagick (`magick`) if installed
    if let Ok(status) = std::process::Command::new("magick")
        .arg(input)
        .arg("-quality")
        .arg(quality.to_string())
        .arg(output)
        .status()
    {
        if status.success() {
            return Ok(());
        }
    }

    anyhow::bail!(
        "HEIC encoding requires 'heif-enc' or 'ImageMagick' installed (HEVC encoders are patent-restricted on Windows). Tip: Use AVIF for royalty-free next-gen image compression with higher quality and smaller file size."
    )
}

pub fn find_heif_enc_path() -> Option<PathBuf> {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let sidecar = PathBuf::from(local_app_data).join("Wheel").join("bin").join("heif-enc.exe");
        if sidecar.exists() {
            return Some(sidecar);
        }
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let next_to = dir.join("heif-enc.exe");
            if next_to.exists() {
                return Some(next_to);
            }
        }
    }
    if std::process::Command::new("heif-enc")
        .arg("-h")
        .output()
        .is_ok()
    {
        return Some(PathBuf::from("heif-enc"));
    }
    None
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
        assert_eq!(OutputFormat::from_extension("heic"), Some(OutputFormat::Heic));
        assert_eq!(OutputFormat::from_extension("heif"), Some(OutputFormat::Heic));
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
}
