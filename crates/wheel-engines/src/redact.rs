use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::{imageops, GenericImageView, ImageFormat, Rgba};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactMode {
    BlackBar,
    Pixelate,
    Blur,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub mode: RedactMode,
}

/// Irreversibly burn redactions into an image's pixel data.
pub fn redact_image(input: &Path, output: &Path, regions: &[RedactRegion]) -> Result<PathBuf> {
    info!(
        "Burning {} redaction(s) irreversibly into {:?}",
        regions.len(),
        input
    );

    let img = image::open(input)
        .with_context(|| format!("Failed to open image for redaction: {:?}", input))?;

    let (img_w, img_h) = img.dimensions();
    let mut rgba = img.to_rgba8();

    for reg in regions {
        let x0 = reg.x.min(img_w);
        let y0 = reg.y.min(img_h);
        let w = reg.width.min(img_w.saturating_sub(x0));
        let h = reg.height.min(img_h.saturating_sub(y0));

        if w == 0 || h == 0 {
            continue;
        }

        match reg.mode {
            RedactMode::BlackBar => {
                // Permanently overwrite all pixels with pure black
                for y in y0..(y0 + h) {
                    for x in x0..(x0 + w) {
                        rgba.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                    }
                }
            }
            RedactMode::Pixelate => {
                // Irreversibly group pixels into 16x16 average blocks
                let block_size = 16u32;
                for by in (y0..(y0 + h)).step_by(block_size as usize) {
                    for bx in (x0..(x0 + w)).step_by(block_size as usize) {
                        let bw = block_size.min((x0 + w).saturating_sub(bx));
                        let bh = block_size.min((y0 + h).saturating_sub(by));

                        // Calculate average color
                        let mut sum_r: u32 = 0;
                        let mut sum_g: u32 = 0;
                        let mut sum_b: u32 = 0;
                        let total = bw * bh;

                        for py in by..(by + bh) {
                            for px in bx..(bx + bw) {
                                let p = rgba.get_pixel(px, py);
                                sum_r += p[0] as u32;
                                sum_g += p[1] as u32;
                                sum_b += p[2] as u32;
                            }
                        }

                        let avg_p = Rgba([
                            (sum_r / total) as u8,
                            (sum_g / total) as u8,
                            (sum_b / total) as u8,
                            255,
                        ]);

                        for py in by..(by + bh) {
                            for px in bx..(bx + bw) {
                                rgba.put_pixel(px, py, avg_p);
                            }
                        }
                    }
                }
            }
            RedactMode::Blur => {
                // Crop sub-image, apply heavy blur, and paste back
                let sub = imageops::crop_imm(&rgba, x0, y0, w, h).to_image();
                let blurred = imageops::blur(&sub, 12.0);
                imageops::overlay(&mut rgba, &blurred, x0 as i64, y0 as i64);
            }
        }
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = output.with_extension("redact.tmp");
    let format = ImageFormat::from_path(output).unwrap_or(ImageFormat::Png);

    if format == ImageFormat::Jpeg {
        let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
        rgb.save_with_format(&tmp, format)?;
    } else {
        rgba.save_with_format(&tmp, format)?;
    }

    std::fs::rename(&tmp, output)?;

    info!("Redacted image saved successfully to {:?}", output);
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    #[test]
    fn test_burn_redaction_black_bar_and_pixelate() {
        let dir = std::env::temp_dir().join("wheel_redact_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_redact_in.png");
        let output_path = dir.join("test_redact_out.png");

        let mut img = RgbaImage::new(100, 100);
        for p in img.pixels_mut() {
            *p = Rgba([200, 200, 200, 255]);
        }
        img.save(&input_path).unwrap();

        let regions = vec![
            RedactRegion {
                x: 10,
                y: 10,
                width: 30,
                height: 30,
                mode: RedactMode::BlackBar,
            },
            RedactRegion {
                x: 50,
                y: 50,
                width: 40,
                height: 40,
                mode: RedactMode::Pixelate,
            },
        ];

        let result = redact_image(&input_path, &output_path, &regions).unwrap();
        assert_eq!(result, output_path);
        assert!(output_path.exists());

        let out_img = image::open(&output_path).unwrap().to_rgba8();
        // Verify black bar area is pure black (0, 0, 0, 255)
        let p_black = out_img.get_pixel(15, 15);
        assert_eq!(*p_black, Rgba([0, 0, 0, 255]));

        // Outside area remains untouched
        let p_untouched = out_img.get_pixel(95, 5);
        assert_eq!(*p_untouched, Rgba([200, 200, 200, 255]));

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }
}
