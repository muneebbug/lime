use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::{imageops::FilterType, DynamicImage, ImageFormat};
use tracing::info;

/// Parameters for image cropping.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CropParams {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Crop an image to the specified rectangle and save to output_path.
pub fn crop_image(input: &Path, output: &Path, params: &CropParams) -> Result<PathBuf> {
    info!("Cropping image {:?} with params {:?}", input, params);

    let img = image::open(input)
        .with_context(|| format!("Failed to open image for cropping: {:?}", input))?;

    let img_w = img.width();
    let img_h = img.height();

    // Clamp coordinates safely within image boundaries
    let crop_x = params.x.min(img_w);
    let crop_y = params.y.min(img_h);
    let crop_w = params.width.min(img_w.saturating_sub(crop_x)).max(1);
    let crop_h = params.height.min(img_h.saturating_sub(crop_y)).max(1);

    let cropped = img.crop_imm(crop_x, crop_y, crop_w, crop_h);

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = output.with_extension("crop.tmp");
    let format = ImageFormat::from_path(output).unwrap_or(ImageFormat::Png);
    cropped.save_with_format(&tmp, format)
        .with_context(|| format!("Failed to save cropped image to {:?}", tmp))?;

    std::fs::rename(&tmp, output)
        .with_context(|| format!("Failed to rename {:?} -> {:?}", tmp, output))?;

    info!("Cropped image saved successfully to {:?}", output);
    Ok(output.to_path_buf())
}

/// Preset mode for compression
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionPreset {
    Balanced,
    Strong,
}

/// Parameters for image compression.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressParams {
    pub preset: CompressionPreset,
    pub target_size_kb: Option<u64>,
}

/// Compress an image using balanced/strong presets or target file size binary search.
pub fn compress_image(input: &Path, output: &Path, params: &CompressParams) -> Result<PathBuf> {
    info!("Compressing image {:?} with params {:?}", input, params);

    let mut img = image::open(input)
        .with_context(|| format!("Failed to open image for compression: {:?}", input))?;

    let format = ImageFormat::from_path(output).unwrap_or_else(|_| {
        ImageFormat::from_path(input).unwrap_or(ImageFormat::Jpeg)
    });

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let target_bytes = params.target_size_kb.map(|kb| kb * 1024);

    if let Some(target) = target_bytes {
        // Binary search for quality / dimension to hit target size
        let encoded = binary_search_compression(&img, format, target)?;
        let tmp = output.with_extension("comp.tmp");
        std::fs::write(&tmp, &encoded)?;
        std::fs::rename(&tmp, output)?;
        return Ok(output.to_path_buf());
    }

    // Default preset compression
    let quality: u8 = match params.preset {
        CompressionPreset::Balanced => 78,
        CompressionPreset::Strong => {
            // In Strong mode, cap maximum dimension to 1920px if larger
            let max_dim = img.width().max(img.height());
            if max_dim > 1920 {
                let ratio = 1920.0 / max_dim as f32;
                let new_w = (img.width() as f32 * ratio).round() as u32;
                let new_h = (img.height() as f32 * ratio).round() as u32;
                img = img.resize(new_w, new_h, FilterType::Lanczos3);
            }
            60
        }
    };

    let tmp = output.with_extension("comp.tmp");
    save_with_quality(&img, format, quality, &tmp)?;
    std::fs::rename(&tmp, output)?;

    info!("Compressed image successfully saved to {:?}", output);
    Ok(output.to_path_buf())
}

fn save_with_quality(img: &DynamicImage, format: ImageFormat, quality: u8, dest: &Path) -> Result<()> {
    match format {
        ImageFormat::Jpeg => {
            let rgb = img.to_rgb8();
            let mut file = std::fs::File::create(dest)?;
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
            rgb.write_with_encoder(encoder)?;
        }
        _ => {
            // For WebP/PNG/etc. save with format
            img.save_with_format(dest, format)?;
        }
    }
    Ok(())
}

fn binary_search_compression(img: &DynamicImage, format: ImageFormat, target_bytes: u64) -> Result<Vec<u8>> {
    let mut current_img = img.clone();

    // Iterate up to 3 dimension downscale rounds if needed
    for _ in 0..3 {
        let mut low = 10u8;
        let mut high = 95u8;
        let mut best_buf = Vec::new();

        while low <= high {
            let mid = (low + high) / 2;
            let mut buf = std::io::Cursor::new(Vec::new());

            if format == ImageFormat::Jpeg {
                let rgb = current_img.to_rgb8();
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, mid);
                let _ = rgb.write_with_encoder(encoder);
            } else {
                let _ = current_img.write_to(&mut buf, format);
            }

            let bytes = buf.into_inner();
            let size = bytes.len() as u64;

            if size <= target_bytes {
                best_buf = bytes;
                low = mid + 1; // Try higher quality
            } else {
                if mid <= 10 {
                    break;
                }
                high = mid - 1; // Try lower quality
            }
        }

        if !best_buf.is_empty() {
            return Ok(best_buf);
        }

        // If even at lowest quality it's too big, resize image by 0.75x
        let new_w = (current_img.width() as f32 * 0.75).round() as u32;
        let new_h = (current_img.height() as f32 * 0.75).round() as u32;
        if new_w < 50 || new_h < 50 {
            break;
        }
        current_img = current_img.resize(new_w, new_h, FilterType::Lanczos3);
    }

    // Fallback: lowest quality encoding
    let mut buf = std::io::Cursor::new(Vec::new());
    let rgb = current_img.to_rgb8();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 15);
    rgb.write_with_encoder(encoder)?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn test_crop_image() {
        let dir = std::env::temp_dir().join("wheel_crop_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_crop_in.png");
        let output_path = dir.join("test_crop_out.png");

        let mut img = RgbaImage::new(200, 200);
        for p in img.pixels_mut() {
            *p = Rgba([100, 150, 200, 255]);
        }
        img.save(&input_path).unwrap();

        let params = CropParams {
            x: 50,
            y: 50,
            width: 80,
            height: 60,
        };

        let result = crop_image(&input_path, &output_path, &params).unwrap();
        assert_eq!(result, output_path);

        let cropped = image::open(&output_path).unwrap();
        assert_eq!(cropped.width(), 80);
        assert_eq!(cropped.height(), 60);

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }

    #[test]
    fn test_compress_image_presets() {
        let dir = std::env::temp_dir().join("wheel_comp_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_comp_in.jpg");
        let output_path = dir.join("test_comp_out.jpg");

        let mut img = image::RgbImage::new(400, 400);
        for (i, p) in img.pixels_mut().enumerate() {
            *p = image::Rgb([(i % 255) as u8, ((i * 2) % 255) as u8, ((i * 3) % 255) as u8]);
        }
        img.save(&input_path).unwrap();

        let initial_size = std::fs::metadata(&input_path).unwrap().len();

        let params = CompressParams {
            preset: CompressionPreset::Strong,
            target_size_kb: None,
        };

        let result = compress_image(&input_path, &output_path, &params).unwrap();
        assert_eq!(result, output_path);
        let compressed_size = std::fs::metadata(&output_path).unwrap().len();
        assert!(compressed_size > 0);
        assert!(compressed_size <= initial_size);

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }
}
