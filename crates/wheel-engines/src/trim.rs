use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat};
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrimBounds {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
    pub original_width: u32,
    pub original_height: u32,
    pub trimmed_width: u32,
    pub trimmed_height: u32,
    pub needs_trim: bool,
}

#[derive(Debug, Clone)]
pub struct TrimResult {
    pub output_path: PathBuf,
    pub bounds: TrimBounds,
}

/// Calculate the minimal bounding box of non-blank (alpha > 0) pixels,
/// identical to Photoshop's "Trim: Based On Transparent Pixels".
pub fn find_trim_bounds(rgba: &image::RgbaImage) -> TrimBounds {
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return TrimBounds {
            min_x: 0,
            min_y: 0,
            max_x: 0,
            max_y: 0,
            original_width: width,
            original_height: height,
            trimmed_width: 0,
            trimmed_height: 0,
            needs_trim: false,
        };
    }

    // 1. Scan from top down to find min_y
    let mut top = 0;
    while top < height {
        let mut row_has_pixel = false;
        for x in 0..width {
            if rgba.get_pixel(x, top)[3] > 0 {
                row_has_pixel = true;
                break;
            }
        }
        if row_has_pixel {
            break;
        }
        top += 1;
    }

    // If all rows are transparent, image is completely blank
    if top == height {
        return TrimBounds {
            min_x: 0,
            min_y: 0,
            max_x: 0,
            max_y: 0,
            original_width: width,
            original_height: height,
            trimmed_width: 1,
            trimmed_height: 1,
            needs_trim: width > 1 || height > 1,
        };
    }

    let min_y = top;

    // 2. Scan from bottom up to find max_y
    let mut bottom = height - 1;
    while bottom >= min_y {
        let mut row_has_pixel = false;
        for x in 0..width {
            if rgba.get_pixel(x, bottom)[3] > 0 {
                row_has_pixel = true;
                break;
            }
        }
        if row_has_pixel {
            break;
        }
        if bottom == 0 {
            break;
        }
        bottom -= 1;
    }
    let max_y = bottom;

    // 3. Scan from left to right to find min_x (only rows between min_y and max_y)
    let mut left = 0;
    while left < width {
        let mut col_has_pixel = false;
        for y in min_y..=max_y {
            if rgba.get_pixel(left, y)[3] > 0 {
                col_has_pixel = true;
                break;
            }
        }
        if col_has_pixel {
            break;
        }
        left += 1;
    }
    let min_x = left;

    // 4. Scan from right to left to find max_x (only rows between min_y and max_y)
    let mut right = width - 1;
    while right >= min_x {
        let mut col_has_pixel = false;
        for y in min_y..=max_y {
            if rgba.get_pixel(right, y)[3] > 0 {
                col_has_pixel = true;
                break;
            }
        }
        if col_has_pixel {
            break;
        }
        if right == 0 {
            break;
        }
        right -= 1;
    }
    let max_x = right;

    let trimmed_width = max_x - min_x + 1;
    let trimmed_height = max_y - min_y + 1;
    let needs_trim = min_x > 0 || min_y > 0 || max_x + 1 < width || max_y + 1 < height;

    TrimBounds {
        min_x,
        min_y,
        max_x,
        max_y,
        original_width: width,
        original_height: height,
        trimmed_width,
        trimmed_height,
        needs_trim,
    }
}

/// Trims all blank (transparent) pixels from the input image and saves to output.
/// Matches Photoshop's Trim feature (Transparent Pixels).
pub fn trim_image(input: &Path, output: &Path) -> Result<TrimResult> {
    info!("Trimming blank pixels: {:?} -> {:?}", input, output);

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create destination directory: {:?}", parent))?;
    }

    let img = match image::open(input) {
        Ok(i) => i,
        Err(err) => {
            // If image::open failed on an exotic format, try FFmpeg decode to temporary PNG
            if let Some(ffmpeg) = crate::media::find_ffmpeg_path() {
                let temp_dir = std::env::temp_dir().join("wheel_temp");
                let _ = std::fs::create_dir_all(&temp_dir);
                let tmp_png = temp_dir.join(format!(
                    "trim_decode_{}.png",
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
                            return trim_loaded_image(loaded, output);
                        }
                    }
                }
                let _ = std::fs::remove_file(&tmp_png);
            }
            return Err(err).with_context(|| format!("Failed to open image for trimming: {:?}", input));
        }
    };

    trim_loaded_image(img, output)
}

fn trim_loaded_image(img: DynamicImage, output: &Path) -> Result<TrimResult> {
    let rgba = img.to_rgba8();
    let bounds = find_trim_bounds(&rgba);

    let cropped_rgba = if bounds.needs_trim {
        image::imageops::crop_imm(
            &rgba,
            bounds.min_x,
            bounds.min_y,
            bounds.trimmed_width,
            bounds.trimmed_height,
        )
        .to_image()
    } else {
        rgba
    };

    let cropped_dynamic = DynamicImage::ImageRgba8(cropped_rgba);

    // Save with atomic replacement
    let temp_dir = std::env::temp_dir().join("wheel_temp");
    let _ = std::fs::create_dir_all(&temp_dir);
    let out_ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    let tmp_png = temp_dir.join(format!(
        "trim_tmp_{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    // Save cropped lossless PNG representation first
    cropped_dynamic
        .save_with_format(&tmp_png, ImageFormat::Png)
        .with_context(|| format!("Failed to encode trimmed PNG to {:?}", tmp_png))?;

    if out_ext == "png" {
        // Atomic replace or copy
        if let Err(e) = std::fs::rename(&tmp_png, output) {
            std::fs::copy(&tmp_png, output)
                .with_context(|| format!("Failed to copy trimmed image to {:?}: {}", output, e))?;
            let _ = std::fs::remove_file(&tmp_png);
        }
    } else if let Some(fmt) = crate::image_convert::OutputFormat::from_extension(&out_ext) {
        // Run through convert_image to properly encode WEBP, AVIF, TIFF, BMP, JPG, etc.
        let params = crate::image_convert::ConvertParams {
            output_format: fmt,
            output_path: output.to_path_buf(),
            quality: 100,
        };
        crate::image_convert::convert_image(&tmp_png, &params)
            .with_context(|| format!("Failed to convert trimmed image to {:?}", output))?;
        let _ = std::fs::remove_file(&tmp_png);
    } else {
        // Fallback directly using DynamicImage save
        cropped_dynamic
            .save(output)
            .with_context(|| format!("Failed to save trimmed image to {:?}", output))?;
        let _ = std::fs::remove_file(&tmp_png);
    }

    info!(
        "Trim complete: {}x{} -> {}x{} (trimmed={})",
        bounds.original_width,
        bounds.original_height,
        bounds.trimmed_width,
        bounds.trimmed_height,
        bounds.needs_trim
    );

    Ok(TrimResult {
        output_path: output.to_path_buf(),
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_trim_transparent_padding() {
        // 100x100 image, mostly transparent
        let mut rgba = image::RgbaImage::new(100, 100);

        // Put a 20x30 rectangle of visible pixels at x: [25, 44], y: [15, 44]
        for y in 15..=44 {
            for x in 25..=44 {
                rgba.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let bounds = find_trim_bounds(&rgba);
        assert!(bounds.needs_trim);
        assert_eq!(bounds.min_x, 25);
        assert_eq!(bounds.max_x, 44);
        assert_eq!(bounds.min_y, 15);
        assert_eq!(bounds.max_y, 44);
        assert_eq!(bounds.trimmed_width, 20);
        assert_eq!(bounds.trimmed_height, 30);
    }

    #[test]
    fn test_trim_no_trim_needed() {
        // 50x50 fully opaque image
        let mut rgba = image::RgbaImage::new(50, 50);
        for y in 0..50 {
            for x in 0..50 {
                rgba.put_pixel(x, y, Rgba([0, 255, 0, 255]));
            }
        }

        let bounds = find_trim_bounds(&rgba);
        assert!(!bounds.needs_trim);
        assert_eq!(bounds.min_x, 0);
        assert_eq!(bounds.min_y, 0);
        assert_eq!(bounds.max_x, 49);
        assert_eq!(bounds.max_y, 49);
        assert_eq!(bounds.trimmed_width, 50);
        assert_eq!(bounds.trimmed_height, 50);
    }

    #[test]
    fn test_trim_completely_transparent() {
        let rgba = image::RgbaImage::new(50, 50);
        let bounds = find_trim_bounds(&rgba);
        assert!(bounds.needs_trim);
        assert_eq!(bounds.trimmed_width, 1);
        assert_eq!(bounds.trimmed_height, 1);
    }
}
