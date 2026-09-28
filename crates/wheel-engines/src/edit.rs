use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::{imageops, ImageFormat};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditParams {
    /// Brightness adjustment (-100 to 100)
    pub brightness: i32,
    /// Contrast adjustment (-100.0 to 100.0)
    pub contrast: f32,
    /// Rotation in degrees (0, 90, 180, 270)
    pub rotation: u32,
    /// Horizontal flip
    pub flip_h: bool,
    /// Vertical flip
    pub flip_v: bool,
    /// Optional target dimensions
    pub resize_w: Option<u32>,
    pub resize_h: Option<u32>,
}

/// Apply photo editing adjustments to an image.
pub fn edit_image(input: &Path, output: &Path, params: &EditParams) -> Result<PathBuf> {
    info!("Editing image {:?} with params {:?}", input, params);

    let mut img = image::open(input)
        .with_context(|| format!("Failed to open image for editing: {:?}", input))?;

    // Rotation
    match params.rotation % 360 {
        90 => img = img.rotate90(),
        180 => img = img.rotate180(),
        270 => img = img.rotate270(),
        _ => {}
    }

    // Flips
    if params.flip_h {
        img = img.fliph();
    }
    if params.flip_v {
        img = img.flipv();
    }

    // Resize
    if let (Some(w), Some(h)) = (params.resize_w, params.resize_h) {
        if w > 0 && h > 0 && (w != img.width() || h != img.height()) {
            img = img.resize_exact(w, h, imageops::FilterType::Lanczos3);
        }
    }

    // Brightness (-100 to 100 map to approx pixel delta)
    if params.brightness != 0 {
        let b = (params.brightness as f32 * 2.55).round() as i32;
        img = img.brighten(b);
    }

    // Contrast
    if params.contrast != 0.0 {
        img = img.adjust_contrast(params.contrast);
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = output.with_extension("edit.tmp");
    let format = ImageFormat::from_path(output).unwrap_or(ImageFormat::Png);

    if format == ImageFormat::Jpeg && img.color().has_alpha() {
        let rgb = img.to_rgb8();
        rgb.save_with_format(&tmp, format)?;
    } else {
        img.save_with_format(&tmp, format)?;
    }

    std::fs::rename(&tmp, output)?;

    info!("Saved edited image to {:?}", output);
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn test_edit_image_rotation_and_adjustments() {
        let dir = std::env::temp_dir().join("wheel_edit_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_edit_in.png");
        let output_path = dir.join("test_edit_out.png");

        let mut img = RgbaImage::new(100, 60);
        for p in img.pixels_mut() {
            *p = Rgba([120, 120, 120, 255]);
        }
        img.save(&input_path).unwrap();

        let params = EditParams {
            brightness: 10,
            contrast: 15.0,
            rotation: 90,
            flip_h: true,
            flip_v: false,
            resize_w: None,
            resize_h: None,
        };

        let result = edit_image(&input_path, &output_path, &params).unwrap();
        assert_eq!(result, output_path);

        let edited = image::open(&output_path).unwrap();
        // 100x60 rotated 90 deg becomes 60x100
        assert_eq!(edited.width(), 60);
        assert_eq!(edited.height(), 100);

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }
}
