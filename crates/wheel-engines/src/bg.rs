use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddBgParams {
    pub padding: u32,
    pub corner_radius: u32,
    pub shadow_blur: u32,
    pub aspect_ratio: String, // "auto", "1:1", "16:9", "4:5"
    pub color_start: [u8; 4], // RGBA
    pub color_end: [u8; 4],   // RGBA
    pub format: String,       // "png", "jpg", "webp"
}

/// Composite an image on top of a styled background canvas with rounded corners and shadow.
pub fn add_background(input: &Path, output: &Path, params: &AddBgParams) -> Result<PathBuf> {
    info!("Adding background to {:?} with params {:?}", input, params);

    let src = image::open(input)
        .with_context(|| format!("Failed to open input image: {:?}", input))?;

    let src_w = src.width();
    let src_h = src.height();

    let pad = params.padding;
    let content_w = src_w + pad * 2;
    let content_h = src_h + pad * 2;

    // Determine target canvas dimensions based on aspect ratio
    let (canvas_w, canvas_h) = match params.aspect_ratio.as_str() {
        "1:1" => {
            let max_dim = content_w.max(content_h);
            (max_dim, max_dim)
        }
        "16:9" => {
            let target_w = (content_h as f32 * (16.0 / 9.0)).round() as u32;
            let final_w = content_w.max(target_w);
            let final_h = (final_w as f32 * (9.0 / 16.0)).round() as u32;
            (final_w, final_h)
        }
        "4:5" => {
            let target_w = (content_h as f32 * (4.0 / 5.0)).round() as u32;
            let final_w = content_w.max(target_w);
            let final_h = (final_w as f32 * (5.0 / 4.0)).round() as u32;
            (final_w, final_h)
        }
        _ => (content_w, content_h), // "auto"
    };

    let mut canvas = RgbaImage::new(canvas_w, canvas_h);

    // Render linear gradient background (top-left to bottom-right)
    let c1 = params.color_start;
    let c2 = params.color_end;

    for y in 0..canvas_h {
        for x in 0..canvas_w {
            let t = ((x as f32 / canvas_w as f32) + (y as f32 / canvas_h as f32)) * 0.5;
            let r = (c1[0] as f32 * (1.0 - t) + c2[0] as f32 * t).round() as u8;
            let g = (c1[1] as f32 * (1.0 - t) + c2[1] as f32 * t).round() as u8;
            let b = (c1[2] as f32 * (1.0 - t) + c2[2] as f32 * t).round() as u8;
            let a = (c1[3] as f32 * (1.0 - t) + c2[3] as f32 * t).round() as u8;
            canvas.put_pixel(x, y, Rgba([r, g, b, a]));
        }
    }

    // Offset to center the source image
    let offset_x = (canvas_w.saturating_sub(src_w)) / 2;
    let offset_y = (canvas_h.saturating_sub(src_h)) / 2;

    // Render drop shadow if requested
    if params.shadow_blur > 0 {
        let shadow_pad = params.shadow_blur * 2;
        let shadow_alpha_base = 90u8;

        for dy in 0..(src_h + shadow_pad) {
            for dx in 0..(src_w + shadow_pad) {
                let sx = offset_x + dx.saturating_sub(params.shadow_blur);
                let sy = offset_y + dy.saturating_sub(params.shadow_blur) + (params.shadow_blur / 2);

                if sx < canvas_w && sy < canvas_h {
                    let cur_pixel = canvas.get_pixel(sx, sy);
                    let dist_x = (dx as i32 - (src_w as i32 / 2)).abs() as f32 / (src_w as f32 / 2.0);
                    let dist_y = (dy as i32 - (src_h as i32 / 2)).abs() as f32 / (src_h as f32 / 2.0);
                    let dist = (dist_x.powi(2) + dist_y.powi(2)).sqrt().min(1.0);

                    let alpha_factor = (1.0 - dist).max(0.0).powi(2);
                    let shadow_a = (shadow_alpha_base as f32 * alpha_factor) as u8;

                    // Alpha blend black shadow into canvas
                    let blend_t = shadow_a as f32 / 255.0;
                    let nr = (cur_pixel[0] as f32 * (1.0 - blend_t)) as u8;
                    let ng = (cur_pixel[1] as f32 * (1.0 - blend_t)) as u8;
                    let nb = (cur_pixel[2] as f32 * (1.0 - blend_t)) as u8;
                    canvas.put_pixel(sx, sy, Rgba([nr, ng, nb, 255]));
                }
            }
        }
    }

    // Prepare rounded corner mask for source image
    let src_rgba = src.to_rgba8();
    let r = params.corner_radius as f32;

    for y in 0..src_h {
        for x in 0..src_w {
            let px = canvas_w.saturating_sub(src_w) / 2 + x;
            let py = canvas_h.saturating_sub(src_h) / 2 + y;

            if px >= canvas_w || py >= canvas_h {
                continue;
            }

            // Check corner rounding
            let mut inside = true;
            if r > 0.0 {
                let check_corner = |cx: f32, cy: f32| -> bool {
                    let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                    d <= r
                };

                if (x as f32) < r && (y as f32) < r {
                    inside = check_corner(r, r);
                } else if (x as f32) >= (src_w as f32 - r) && (y as f32) < r {
                    inside = check_corner(src_w as f32 - r, r);
                } else if (x as f32) < r && (y as f32) >= (src_h as f32 - r) {
                    inside = check_corner(r, src_h as f32 - r);
                } else if (x as f32) >= (src_w as f32 - r) && (y as f32) >= (src_h as f32 - r) {
                    inside = check_corner(src_w as f32 - r, src_h as f32 - r);
                }
            }

            if inside {
                let sp = src_rgba.get_pixel(x, y);
                if sp[3] == 255 {
                    canvas.put_pixel(px, py, *sp);
                } else if sp[3] > 0 {
                    let bg_p = canvas.get_pixel(px, py);
                    let alpha = sp[3] as f32 / 255.0;
                    let nr = (sp[0] as f32 * alpha + bg_p[0] as f32 * (1.0 - alpha)).round() as u8;
                    let ng = (sp[1] as f32 * alpha + bg_p[1] as f32 * (1.0 - alpha)).round() as u8;
                    let nb = (sp[2] as f32 * alpha + bg_p[2] as f32 * (1.0 - alpha)).round() as u8;
                    canvas.put_pixel(px, py, Rgba([nr, ng, nb, 255]));
                }
            }
        }
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = output.with_extension("bg.tmp");
    let format = match params.format.to_lowercase().as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    if format == ImageFormat::Jpeg {
        let rgb = DynamicImage::ImageRgba8(canvas).to_rgb8();
        rgb.save_with_format(&tmp, format)?;
    } else {
        canvas.save_with_format(&tmp, format)?;
    }

    std::fs::rename(&tmp, output)?;

    info!("Saved backdrop image to {:?}", output);
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_background_composition() {
        let dir = std::env::temp_dir().join("wheel_bg_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_bg_in.png");
        let output_path = dir.join("test_bg_out.png");

        let mut img = RgbaImage::new(100, 100);
        for p in img.pixels_mut() {
            *p = Rgba([255, 255, 255, 255]);
        }
        img.save(&input_path).unwrap();

        let params = AddBgParams {
            padding: 30,
            corner_radius: 12,
            shadow_blur: 10,
            aspect_ratio: "1:1".into(),
            color_start: [249, 115, 22, 255], // Orange
            color_end: [236, 72, 153, 255],   // Pink
            format: "png".into(),
        };

        let result = add_background(&input_path, &output_path, &params).unwrap();
        assert_eq!(result, output_path);
        assert!(output_path.exists());

        let out_img = image::open(&output_path).unwrap();
        assert_eq!(out_img.width(), out_img.height(), "1:1 ratio must be square");
        assert!(out_img.width() >= 160);

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }
}
