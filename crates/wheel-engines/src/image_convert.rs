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
    // Heic, Avif, Pdf — added in M2
}

impl OutputFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "bmp" => Some(Self::Bmp),
            "tiff" | "tif" => Some(Self::Tiff),
            _ => None,
        }
    }

    pub fn image_format(&self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Webp => ImageFormat::WebP,
            Self::Bmp => ImageFormat::Bmp,
            Self::Tiff => ImageFormat::Tiff,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
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
    let img = image::open(input)
        .with_context(|| format!("Failed to open {:?}", input))?;

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
        fmt => {
            img.save_with_format(&tmp, fmt.image_format())
                .with_context(|| format!("Encode to {:?} failed", fmt))?;
        }
    }

    std::fs::rename(&tmp, &params.output_path)
        .with_context(|| format!("Atomic rename {:?} -> {:?} failed", tmp, params.output_path))?;

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
        assert_eq!(OutputFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_convert_image_roundtrip() {
        use image::{RgbaImage, Rgba};
        let dir = std::env::temp_dir().join("wheel_tests");
        let _ = std::fs::create_dir_all(&dir);

        let src_path = dir.join("test_input.png");
        let dst_path = dir.join("test_output.webp");

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

        // Clean up
        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(dst_path);
    }
}
