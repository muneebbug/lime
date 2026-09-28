use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::{imageops, GenericImageView, ImageFormat, Rgba};
use tracing::info;

/// Apply an annotation overlay (drawn at full image resolution) on top of an image.
pub fn apply_annotation_overlay(
    input: &Path,
    output: &Path,
    overlay_bytes: &[u8],
) -> Result<PathBuf> {
    info!("Compositing annotation overlay onto {:?}", input);

    let base = image::open(input)
        .with_context(|| format!("Failed to open base image: {:?}", input))?;

    let overlay = image::load_from_memory(overlay_bytes)
        .with_context(|| "Failed to load annotation overlay from memory")?;

    let (base_w, base_h) = base.dimensions();
    let mut base_rgba = base.to_rgba8();

    // Scale overlay if canvas DPI differed from base image
    let overlay_rgba = if overlay.width() != base_w || overlay.height() != base_h {
        overlay.resize_exact(base_w, base_h, imageops::FilterType::Lanczos3).to_rgba8()
    } else {
        overlay.to_rgba8()
    };

    // Alpha blend overlay onto base image
    for y in 0..base_h {
        for x in 0..base_w {
            let op = overlay_rgba.get_pixel(x, y);
            if op[3] == 255 {
                base_rgba.put_pixel(x, y, *op);
            } else if op[3] > 0 {
                let bp = base_rgba.get_pixel(x, y);
                let a = op[3] as f32 / 255.0;
                let nr = (op[0] as f32 * a + bp[0] as f32 * (1.0 - a)).round() as u8;
                let ng = (op[1] as f32 * a + bp[1] as f32 * (1.0 - a)).round() as u8;
                let nb = (op[2] as f32 * a + bp[2] as f32 * (1.0 - a)).round() as u8;
                base_rgba.put_pixel(x, y, Rgba([nr, ng, nb, 255]));
            }
        }
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = output.with_extension("ann.tmp");
    let format = ImageFormat::from_path(output).unwrap_or(ImageFormat::Png);

    if format == ImageFormat::Jpeg {
        let rgb = image::DynamicImage::ImageRgba8(base_rgba).to_rgb8();
        rgb.save_with_format(&tmp, format)?;
    } else {
        base_rgba.save_with_format(&tmp, format)?;
    }

    std::fs::rename(&tmp, output)?;

    info!("Annotated image saved successfully to {:?}", output);
    Ok(output.to_path_buf())
}

/// Apply an annotation overlay encoded as base64 or a data-URI string onto the base image.
pub fn apply_annotation_overlay_base64(
    input: &Path,
    output: &Path,
    base64_data: &str,
) -> Result<PathBuf> {
    use base64::Engine;
    let clean_b64 = if let Some(idx) = base64_data.find(',') {
        &base64_data[idx + 1..]
    } else {
        base64_data
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(clean_b64.trim())
        .with_context(|| "Failed to decode base64 annotation overlay data")?;

    apply_annotation_overlay(input, output, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    #[test]
    fn test_apply_annotation_overlay() {
        let dir = std::env::temp_dir().join("wheel_ann_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_ann_in.png");
        let output_path = dir.join("test_ann_out.png");

        let mut img = RgbaImage::new(100, 100);
        for p in img.pixels_mut() {
            *p = Rgba([50, 50, 50, 255]);
        }
        img.save(&input_path).unwrap();

        // Create a 100x100 overlay with a red line
        let mut overlay = RgbaImage::new(100, 100);
        for x in 10..90 {
            overlay.put_pixel(x, 50, Rgba([255, 0, 0, 255]));
        }
        let mut overlay_buf = std::io::Cursor::new(Vec::new());
        overlay.write_to(&mut overlay_buf, ImageFormat::Png).unwrap();

        let result = apply_annotation_overlay(&input_path, &output_path, &overlay_buf.into_inner()).unwrap();
        assert_eq!(result, output_path);
        assert!(output_path.exists());

        let out_img = image::open(&output_path).unwrap().to_rgba8();
        // Pixel on the annotated line is red
        assert_eq!(*out_img.get_pixel(50, 50), Rgba([255, 0, 0, 255]));
        // Pixel outside is base gray
        assert_eq!(*out_img.get_pixel(50, 20), Rgba([50, 50, 50, 255]));

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }
}
