//! What the presets actually do to real files.
//!
//!   cargo test --release -p wheel-engines compress_report -- --ignored --nocapture
//!
//! Printed rather than asserted: the point is the numbers, and the right figures
//! depend on the machine and the image.

use std::path::{Path, PathBuf};

use wheel_engines::compress::{
    compress_file, AdvancedOptions, CompressParams, CompressPreset, CompressResult,
};

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("wheel_compress_report");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A noisy, photo-like image, so lossy has real work to do.
fn source(dir: &Path, name: &str, width: u32, height: u32) -> PathBuf {
    let path = dir.join(name);
    let mut img = image::RgbaImage::new(width, height);
    let mut state: u32 = 0xC0FFEE;
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (state >> 24) as u8;
        // A smooth gradient plus noise, which is what a photo is.
        let base = ((x * 255 / width.max(1)) / 2 + (y * 255 / height.max(1)) / 2) as u8;
        *pixel = image::Rgba([
            noise.wrapping_add(base) / 2 + 40,
            noise.wrapping_add(base) / 2 + 80,
            noise.wrapping_add(base) / 2 + 120,
            255,
        ]);
    }
    img.save(&path).unwrap();
    path
}

fn report(label: &str, result: &CompressResult, elapsed_ms: u128) {
    let pct = (result.ratio() * 100.0).round();
    if result.unchanged {
        println!("    {label:<22} already smallest  ({} bytes)", result.original_size);
    } else {
        println!(
            "    {label:<22} {:>9} -> {:>9}  ({pct:>3}% smaller, {elapsed_ms:>5} ms)",
            result.original_size,
            result.output_size
        );
    }
}

#[test]
#[ignore = "prints a report; run deliberately"]
fn compress_report() {
    let dir = temp_dir();

    for (name, w, h) in [("photo.png", 1600, 1200), ("flat.png", 1200, 800)] {
        let src = source(&dir, name, w, h);
        println!("\n{name}  ({} x {})", w, h);

        for (label, params) in [
            ("Balanced", CompressParams::default()),
            (
                "Strong",
                CompressParams {
                    preset: CompressPreset::Strong,
                    ..Default::default()
                },
            ),
            (
                "Maximal (zopfli)",
                CompressParams {
                    preset: CompressPreset::Maximal,
                    ..Default::default()
                },
            ),
            (
                "Resize to 1920",
                CompressParams {
                    max_dimension: Some(1920),
                    ..Default::default()
                },
            ),
            (
                "Strong + keep metadata",
                CompressParams {
                    preset: CompressPreset::Strong,
                    keep_metadata: true,
                    ..Default::default()
                },
            ),
        ] {
            let out = dir.join(format!("out-{}-{label}.png", name));
            let start = std::time::Instant::now();
            match compress_file(&src, &out, &params) {
                Ok(result) => report(label, &result, start.elapsed().as_millis()),
                Err(e) => println!("    {label:<22} failed: {e}"),
            }
        }

        // Target size, which is the feature that needs the search loop.
        for mb in [2u64, 1] {
            let out = dir.join(format!("out-{name}-target{mb}.png"));
            let start = std::time::Instant::now();
            match compress_file(
                &src,
                &out,
                &CompressParams {
                    target_size: Some(mb * 1024 * 1024),
                    ..Default::default()
                },
            ) {
                Ok(result) => {
                    let note = if result.target_missed {
                        "  (target NOT met)"
                    } else {
                        "  (met)"
                    };
                    println!(
                        "    {:<22} {:>9} -> {:>9}  ({:>5} ms){note}",
                        format!("Target {mb} MB"),
                        result.original_size,
                        result.output_size,
                        start.elapsed().as_millis()
                    );
                }
                Err(e) => println!("    Target {mb} MB failed: {e}"),
            }
        }
    }

    // JPEG, since that is where the codec choice matters most.
    let jpeg_src = dir.join("photo.jpg");
    let img = image::open(&dir.join("photo.png")).unwrap();
    img.to_rgb8().save_with_format(&jpeg_src, image::ImageFormat::Jpeg).unwrap();
    println!("\nphoto.jpg (re-encoded from the PNG above)");
    for (label, params) in [
        ("Balanced", CompressParams::default()),
        (
            "Strong + lossless",
            CompressParams {
                preset: CompressPreset::Strong,
                advanced: AdvancedOptions {
                    lossless_optimize: Some(true),
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        (
            "Quality 40",
            CompressParams {
                advanced: AdvancedOptions {
                    quality: Some(40),
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
    ] {
        let out = dir.join(format!("out-photo-{label}.jpg"));
        let start = std::time::Instant::now();
        match compress_file(&jpeg_src, &out, &params) {
            Ok(result) => report(label, &result, start.elapsed().as_millis()),
            Err(e) => println!("    {label:<22} failed: {e}"),
        }
    }

    // A real file from the wild, if it is still there.
    let real = Path::new(r"C:\Users\realm\Downloads\Testing\elephant.webp");
    if real.exists() {
        let out = dir.join("elephant-out.webp");
        println!("\nelephant.webp (real file)");
        for (label, params) in [
            ("Balanced", CompressParams::default()),
            (
                "Strong",
                CompressParams {
                    preset: CompressPreset::Strong,
                    ..Default::default()
                },
            ),
        ] {
            let start = std::time::Instant::now();
            match compress_file(real, &out, &params) {
                Ok(result) => report(label, &result, start.elapsed().as_millis()),
                Err(e) => println!("    {label:<22} failed: {e}"),
            }
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}