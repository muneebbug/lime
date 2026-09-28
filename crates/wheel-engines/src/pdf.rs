use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use lopdf::{dictionary, Object, Stream, Document};
use tracing::info;

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
        let img_stream = match lopdf::xobject::image(input) {
            Ok(s) => s,
            Err(_) => {
                // Fallback: load with image crate, convert to RGB8, and serialize to PNG buffer for lopdf
                let img = image::open(input)
                    .with_context(|| format!("Failed to open image {:?}", input))?;
                let rgb = img.to_rgb8();
                let mut buf = std::io::Cursor::new(Vec::new());
                rgb.write_to(&mut buf, image::ImageFormat::Png)
                    .with_context(|| "Failed to buffer image as PNG")?;
                lopdf::xobject::image_from(buf.into_inner())
                    .with_context(|| "Failed to convert image buffer to PDF XObject")?
            }
        };

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

/// Extract raster images embedded in a PDF document to separate image files.
/// Output format can be "png" or "jpg".
pub fn pdf_to_images(
    input: &Path,
    output_prefix: &Path,
    format: &str,
) -> Result<Vec<PathBuf>> {
    let doc = Document::load(input)
        .with_context(|| format!("Failed to open PDF {:?}", input))?;

    let mut outputs = Vec::new();
    let mut image_count = 0;

    for (_obj_id, object) in doc.objects.iter() {
        if let Object::Stream(ref stream) = *object {
            let is_image = stream
                .dict
                .get(b"Subtype")
                .map(|s| s == &Object::Name(b"Image".to_vec()))
                .unwrap_or(false);

            if is_image {
                image_count += 1;
                let filter = stream.dict.get(b"Filter").ok();
                let width = stream.dict.get(b"Width").and_then(|w| w.as_i64()).unwrap_or(0) as u32;
                let height = stream.dict.get(b"Height").and_then(|h| h.as_i64()).unwrap_or(0) as u32;

                let ext = match format.to_lowercase().as_str() {
                    "jpg" | "jpeg" => "jpg",
                    _ => "png",
                };

                let out_path = if image_count == 1 {
                    output_prefix.with_extension(ext)
                } else {
                    let stem = output_prefix.file_stem().and_then(|s| s.to_str()).unwrap_or("page");
                    output_prefix.with_file_name(format!("{}_{}.{}", stem, image_count, ext))
                };

                let is_dct = match filter {
                    Some(Object::Name(ref name)) => name == b"DCTDecode",
                    _ => false,
                };

                if is_dct && (ext == "jpg" || ext == "jpeg") {
                    std::fs::write(&out_path, &stream.content)
                        .with_context(|| format!("Failed to write JPEG stream to {:?}", out_path))?;
                    outputs.push(out_path);
                } else if is_dct {
                    let img = image::load_from_memory(&stream.content)
                        .with_context(|| "Failed to load JPEG stream into image")?;
                    img.save(&out_path)
                        .with_context(|| format!("Failed to save PNG from JPEG stream to {:?}", out_path))?;
                    outputs.push(out_path);
                } else if let Ok(decompressed) = stream.decompressed_content() {
                    let color_space = stream.dict.get(b"ColorSpace").ok();
                    let is_gray = match color_space {
                        Some(Object::Name(ref name)) => name == b"DeviceGray",
                        _ => false,
                    };

                    if is_gray && (decompressed.len() >= (width * height) as usize) {
                        if let Some(gray_buf) = image::GrayImage::from_raw(width, height, decompressed[..(width * height) as usize].to_vec()) {
                            gray_buf.save(&out_path)
                                .with_context(|| format!("Failed to save gray image to {:?}", out_path))?;
                            outputs.push(out_path);
                        }
                    } else if decompressed.len() >= (width * height * 3) as usize {
                        if let Some(rgb_buf) = image::RgbImage::from_raw(width, height, decompressed[..(width * height * 3) as usize].to_vec()) {
                            rgb_buf.save(&out_path)
                                .with_context(|| format!("Failed to save RGB image to {:?}", out_path))?;
                            outputs.push(out_path);
                        }
                    } else if decompressed.len() >= (width * height * 4) as usize {
                        if let Some(rgba_buf) = image::RgbaImage::from_raw(width, height, decompressed[..(width * height * 4) as usize].to_vec()) {
                            rgba_buf.save(&out_path)
                                .with_context(|| format!("Failed to save RGBA image to {:?}", out_path))?;
                            outputs.push(out_path);
                        }
                    }
                }
            }
        }
    }

    if outputs.is_empty() {
        anyhow::bail!("No extractable raster images found in PDF");
    }

    info!("Extracted {} images from PDF: {:?}", outputs.len(), input);
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    #[test]
    fn test_images_to_pdf_creation_and_extraction() {
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

        // Test extracting back to images
        let extract_prefix = dir.join("extracted");
        let extracted = pdf_to_images(&pdf_path, &extract_prefix, "png").unwrap();
        assert!(!extracted.is_empty(), "Should extract at least 1 image");

        // Clean up
        let _ = std::fs::remove_file(img1_path);
        let _ = std::fs::remove_file(img2_path);
        let _ = std::fs::remove_file(pdf_path);
        for f in extracted {
            let _ = std::fs::remove_file(f);
        }
    }
}
