use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use tracing::info;

/// Load an image file and create its PDF Image XObject(s).
/// If the image has transparency (alpha channel < 255), this creates:
/// 1. A 1-channel DeviceGray soft mask (`/SMask`) XObject for the alpha channel.
/// 2. A 3-channel DeviceRGB image XObject flattened onto pure white (`[255, 255, 255]`)
///    with `/Matte [1 1 1]` and `/SMask` referencing the mask.
///
/// This eliminates black background artifacts and dark fringe halos on transparent PNGs,
/// ensuring crisp rendering in all PDF viewers, printers, and thumbnail generators.
fn load_image_xobject(
    doc: &mut Document,
    input: &Path,
) -> Result<(ObjectId, i64, i64)> {
    let buffer = std::fs::read(input)
        .with_context(|| format!("Failed to read image file {:?}", input))?;

    let is_jpeg = image::guess_format(&buffer)
        .map(|fmt| fmt == image::ImageFormat::Jpeg)
        .unwrap_or(false);

    // Fast-path for JPEGs: direct DCT stream without recompression
    if is_jpeg {
        if let Ok(img_stream) = lopdf::xobject::image(input) {
            let width = img_stream
                .dict
                .get(b"Width")
                .map_err(|e| anyhow::anyhow!("Missing Width in image dict: {:?}", e))?
                .as_i64()
                .map_err(|e| anyhow::anyhow!("Invalid Width in image dict: {:?}", e))?;
            let height = img_stream
                .dict
                .get(b"Height")
                .map_err(|e| anyhow::anyhow!("Missing Height in image dict: {:?}", e))?
                .as_i64()
                .map_err(|e| anyhow::anyhow!("Invalid Height in image dict: {:?}", e))?;
            let img_id = doc.add_object(img_stream);
            return Ok((img_id, width, height));
        }
    }

    // Decode image using unified loader (supports standard formats, SVG, and FFmpeg fallback)
    let img = crate::image_convert::load_image(input)?;

    let width = img.width() as i64;
    let height = img.height() as i64;
    let rgba = img.to_rgba8();
    let has_transparency = rgba.pixels().any(|p| p[3] < 255);

    if has_transparency {
        // 1. Create Soft Mask (SMask) 8-bit grayscale stream
        let alpha_bytes: Vec<u8> = rgba.pixels().map(|p| p[3]).collect();
        let smask_dict = dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => width,
            "Height" => height,
            "ColorSpace" => "DeviceGray",
            "BitsPerComponent" => 8,
            "Matte" => vec![1.into(), 1.into(), 1.into()],
        };
        let mut smask_stream = Stream::new(smask_dict, alpha_bytes);
        let _ = smask_stream.compress();
        let smask_id = doc.add_object(smask_stream);

        // 2. Blend RGB image onto pure white [255, 255, 255]
        let rgb = crate::image_convert::flatten_to_rgb(&img, [255, 255, 255]);
        let img_dict = dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => width,
            "Height" => height,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "SMask" => smask_id,
        };
        let mut img_stream = Stream::new(img_dict, rgb.into_raw());
        let _ = img_stream.compress();
        let img_id = doc.add_object(img_stream);

        Ok((img_id, width, height))
    } else {
        let rgb = img.to_rgb8();
        let img_dict = dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => width,
            "Height" => height,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        };
        let mut img_stream = Stream::new(img_dict, rgb.into_raw());
        let _ = img_stream.compress();
        let img_id = doc.add_object(img_stream);

        Ok((img_id, width, height))
    }
}

/// Convert one or more images into a single PDF document.
/// Each image is placed on its own page matching its pixel dimensions.
pub fn images_to_pdf(inputs: &[PathBuf], output_path: &Path) -> Result<PathBuf> {
    if inputs.is_empty() {
        anyhow::bail!("No input files provided for PDF conversion");
    }

    info!("Converting {} image(s) to PDF: {:?}", inputs.len(), output_path);

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for (i, input) in inputs.iter().enumerate() {
        let (img_id, width, height) = load_image_xobject(&mut doc, input)?;
        let img_name = format!("Im{}", i);

        // Content stream: paint image onto page with full dimensions
        let content_bytes = format!("q {} 0 0 {} 0 0 cm /{} Do Q", width, height, img_name).into_bytes();
        let content_stream = Stream::new(dictionary!(), content_bytes);
        let content_id = doc.add_object(content_stream);

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), (width as f64).into(), (height as f64).into()],
            "Contents" => content_id,
            "Resources" => dictionary! {
                "XObject" => dictionary! {
                    img_name => img_id,
                },
            },
        };

        let page_id = doc.add_object(page_dict);
        page_ids.push(Object::Reference(page_id));
    }

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.clone(),
        "Count" => page_ids.len() as i64,
    };
    doc.set_object(pages_id, pages_dict);

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Save to temp file first, then atomic rename
    let tmp = output_path.with_extension("pdf.tmp");
    doc.save(&tmp)
        .with_context(|| format!("Failed to save PDF to {:?}", tmp))?;

    std::fs::rename(&tmp, output_path)
        .with_context(|| format!("Failed to rename {:?} -> {:?}", tmp, output_path))?;

    info!("PDF successfully created: {:?}", output_path);
    Ok(output_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    #[test]
    fn test_images_to_pdf_creation() {
        let dir = std::env::temp_dir().join("wheel_pdf_tests");
        let _ = std::fs::create_dir_all(&dir);

        let img1_path = dir.join("test_img1.png");
        let img2_path = dir.join("test_img2.jpg");
        let pdf_path = dir.join("output.pdf");

        // Create 2 test images: one RGBA PNG, one RGB JPEG
        let mut img1 = RgbaImage::new(100, 50);
        for p in img1.pixels_mut() {
            *p = Rgba([255, 0, 0, 255]);
        }
        img1.save(&img1_path).unwrap();

        let mut img2 = RgbImage::new(80, 80);
        for p in img2.pixels_mut() {
            *p = Rgb([0, 255, 0]);
        }
        img2.save(&img2_path).unwrap();

        let result = images_to_pdf(&[img1_path.clone(), img2_path.clone()], &pdf_path).unwrap();
        assert_eq!(result, pdf_path);
        assert!(pdf_path.exists());
        assert!(std::fs::metadata(&pdf_path).unwrap().len() > 100);

        // Verify with lopdf that the generated PDF can be loaded and has 2 pages
        let loaded = Document::load(&pdf_path).expect("failed to reload generated PDF");
        let pages = loaded.get_pages();
        assert_eq!(pages.len(), 2);

        // Clean up
        let _ = std::fs::remove_file(img1_path);
        let _ = std::fs::remove_file(img2_path);
        let _ = std::fs::remove_file(pdf_path);
    }

    #[test]
    fn test_transparent_png_to_pdf_preserves_clean_white_background_and_smask() {
        let dir = std::env::temp_dir().join("wheel_pdf_transparency_tests");
        let _ = std::fs::create_dir_all(&dir);

        let png_path = dir.join("lime_slice.png");
        let pdf_path = dir.join("lime_slice.pdf");

        // Create an image with transparent background (alpha = 0) and solid green center
        let mut img = RgbaImage::new(40, 40);
        for (x, y, p) in img.enumerate_pixels_mut() {
            if (x >= 10 && x < 30) && (y >= 10 && y < 30) {
                *p = Rgba([150, 255, 0, 255]); // Lime green
            } else {
                *p = Rgba([0, 0, 0, 0]); // Fully transparent with black RGB
            }
        }
        img.save(&png_path).unwrap();

        let result = images_to_pdf(&[png_path.clone()], &pdf_path).unwrap();
        assert_eq!(result, pdf_path);

        // Load document and verify SMask exists
        let loaded = Document::load(&pdf_path).expect("failed to load PDF");
        let mut found_smask = false;
        let mut found_white_preblend = false;

        for (_id, obj) in loaded.objects.iter() {
            if let Object::Stream(ref stream) = *obj {
                if stream.dict.get(b"SMask").is_ok() {
                    found_smask = true;
                    // Check decompressed content: the transparent pixel at (0, 0) MUST be [255, 255, 255] (white), NOT [0, 0, 0] (black)
                    if let Ok(decompressed) = stream.decompressed_content() {
                        let r = decompressed[0];
                        let g = decompressed[1];
                        let b = decompressed[2];
                        if r == 255 && g == 255 && b == 255 {
                            found_white_preblend = true;
                        }
                    }
                }
            }
        }

        assert!(found_smask, "Expected an SMask XObject for transparent PNG");
        assert!(found_white_preblend, "Expected transparent pixels to be pre-blended against pure white, not black!");

        let _ = std::fs::remove_file(png_path);
        let _ = std::fs::remove_file(pdf_path);
    }
}
