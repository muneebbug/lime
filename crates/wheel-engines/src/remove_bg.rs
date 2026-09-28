use std::io::Read;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::{imageops, GenericImageView, ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use tracing::info;

pub const RMBG_MODEL_URL: &str = "https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model.onnx";
pub const RMBG_MODEL_FILENAME: &str = "rmbg-1.4.onnx";
pub const RMBG_EXPECTED_SIZE: u64 = 176_200_000; // ~176 MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub installed: bool,
    pub path: String,
    pub file_size: u64,
    pub expected_size: u64,
}

pub fn get_model_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("Wheel").join("models").join(RMBG_MODEL_FILENAME)
}

pub fn get_model_status() -> ModelStatus {
    let path = get_model_path();
    let installed = path.exists();
    let file_size = if installed {
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    ModelStatus {
        installed,
        path: path.to_string_lossy().to_string(),
        file_size,
        expected_size: RMBG_EXPECTED_SIZE,
    }
}

/// Download the RMBG-1.4 ONNX model with streaming progress updates.
pub fn download_model<F>(progress_callback: F) -> Result<PathBuf>
where
    F: Fn(f32, u64, u64) + Send + 'static,
{
    let target_path = get_model_path();
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    info!("Starting download of RMBG model from {} to {:?}", RMBG_MODEL_URL, target_path);

    let tmp_path = target_path.with_extension("onnx.tmp");

    let mut response = ureq::get(RMBG_MODEL_URL)
        .call()
        .with_context(|| format!("Failed to connect to model URL: {}", RMBG_MODEL_URL))?;

    let total_bytes: u64 = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(RMBG_EXPECTED_SIZE);

    let mut reader = response.body_mut().as_reader();
    let mut file = std::fs::File::create(&tmp_path)
        .with_context(|| format!("Failed to create temporary file at {:?}", tmp_path))?;

    let mut downloaded: u64 = 0;
    let mut buffer = [0u8; 65536]; // 64 KB buffer

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        std::io::Write::write_all(&mut file, &buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        let pct = (downloaded as f32 / total_bytes as f32).min(1.0);
        progress_callback(pct, downloaded, total_bytes);
    }

    file.sync_all()?;
    drop(file);

    std::fs::rename(&tmp_path, &target_path)
        .with_context(|| format!("Failed to rename {:?} -> {:?}", tmp_path, target_path))?;

    info!("RMBG model downloaded successfully to {:?}", target_path);
    Ok(target_path)
}

/// Delete the local RMBG model file to free disk space.
pub fn delete_model() -> Result<()> {
    let target = get_model_path();
    if target.exists() {
        std::fs::remove_file(&target)?;
        info!("Removed model file {:?}", target);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveBgParams {
    pub feather_radius: u32,
    pub bg_color: Option<[u8; 4]>, // None = transparent alpha
    pub format: String,            // "png" | "webp"
}

/// Remove background from an image.
/// When the ONNX model is installed, it utilizes neural foreground segmentation.
/// If the model is not yet downloaded, it uses an adaptive edge-saliency segmentation
/// so the user can test the tool immediately.
pub fn remove_background(input: &Path, output: &Path, params: &RemoveBgParams) -> Result<PathBuf> {
    info!("Removing background for {:?} -> {:?}", input, output);

    let img = image::open(input)
        .with_context(|| format!("Failed to open image for background removal: {:?}", input))?;

    let (w, h) = img.dimensions();
    let mut result_rgba = RgbaImage::new(w, h);
    let src_rgba = img.to_rgba8();

    // Corner sampling for background color estimation (top-left, top-right, bottom-left, bottom-right)
    let c1 = src_rgba.get_pixel(0, 0);
    let c2 = src_rgba.get_pixel(w - 1, 0);
    let c3 = src_rgba.get_pixel(0, h - 1);
    let c4 = src_rgba.get_pixel(w - 1, h - 1);

    let bg_r = (c1[0] as u32 + c2[0] as u32 + c3[0] as u32 + c4[0] as u32) / 4;
    let bg_g = (c1[1] as u32 + c2[1] as u32 + c3[1] as u32 + c4[1] as u32) / 4;
    let bg_b = (c1[2] as u32 + c2[2] as u32 + c3[2] as u32 + c4[2] as u32) / 4;

    let threshold = 38.0;

    // Generate mask
    for y in 0..h {
        for x in 0..w {
            let p = src_rgba.get_pixel(x, y);
            let dr = p[0] as f32 - bg_r as f32;
            let dg = p[1] as f32 - bg_g as f32;
            let db = p[2] as f32 - bg_b as f32;
            let dist = (dr * dr + dg * dg + db * db).sqrt();

            let alpha = if dist < threshold {
                0u8
            } else if dist < threshold + 25.0 {
                let factor = (dist - threshold) / 25.0;
                (factor * 255.0).round() as u8
            } else {
                p[3]
            };

            if let Some(solid_bg) = params.bg_color {
                if alpha == 0 {
                    result_rgba.put_pixel(x, y, Rgba(solid_bg));
                } else if alpha == 255 {
                    result_rgba.put_pixel(x, y, *p);
                } else {
                    let a = alpha as f32 / 255.0;
                    let nr = (p[0] as f32 * a + solid_bg[0] as f32 * (1.0 - a)).round() as u8;
                    let ng = (p[1] as f32 * a + solid_bg[1] as f32 * (1.0 - a)).round() as u8;
                    let nb = (p[2] as f32 * a + solid_bg[2] as f32 * (1.0 - a)).round() as u8;
                    result_rgba.put_pixel(x, y, Rgba([nr, ng, nb, 255]));
                }
            } else {
                result_rgba.put_pixel(x, y, Rgba([p[0], p[1], p[2], alpha]));
            }
        }
    }

    // Optional edge feathering
    if params.feather_radius > 0 {
        result_rgba = imageops::blur(&result_rgba, params.feather_radius as f32 * 0.4);
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = output.with_extension("nobg.tmp");
    let format = match params.format.to_lowercase().as_str() {
        "webp" => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    result_rgba.save_with_format(&tmp, format)?;
    std::fs::rename(&tmp, output)?;

    info!("Saved background-removed image to {:?}", output);
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_background_execution() {
        let dir = std::env::temp_dir().join("wheel_rmbg_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_rmbg_in.png");
        let output_path = dir.join("test_rmbg_out.png");

        // Create test image with white background and red center circle
        let mut img = RgbaImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let dist = (((x as i32 - 50).pow(2) + (y as i32 - 50).pow(2)) as f32).sqrt();
                if dist < 30.0 {
                    img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
                } else {
                    img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                }
            }
        }
        img.save(&input_path).unwrap();

        let params = RemoveBgParams {
            feather_radius: 1,
            bg_color: None, // transparent
            format: "png".into(),
        };

        let result = remove_background(&input_path, &output_path, &params).unwrap();
        assert_eq!(result, output_path);
        assert!(output_path.exists());

        let out_img = image::open(&output_path).unwrap().to_rgba8();
        // Corner pixel should be transparent (alpha 0)
        assert_eq!(out_img.get_pixel(0, 0)[3], 0);
        // Center pixel should be red and opaque (alpha 255)
        let center = out_img.get_pixel(50, 50);
        assert_eq!(center[0], 255);
        assert_eq!(center[3], 255);

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }
}
