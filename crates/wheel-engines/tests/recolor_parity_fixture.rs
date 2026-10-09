//! Generate the shared recolour fixture used to check the Rust engine against the
//! TypeScript preview.
//!
//! Run with `cargo test -p wheel-engines parity_fixture -- --ignored --nocapture`.
//! The output lands in `apps/desktop/src/lib/__tests__/recolor-parity.json`, which
//! `scripts/recolor-parity.mts` replays through the frontend's port of the same
//! maths. If the two ever disagree, the preview is showing something the save
//! will not reproduce.

use image::{Rgba, RgbaImage};
use serde_json::json;
use wheel_engines::recolor::{RecolorParams, RecolorRule, RecolorMode, Rgb};

#[test]
#[ignore = "regenerates a fixture; run deliberately"]
fn parity_fixture() {
    let cases = build_cases();

    let fixture: Vec<_> = cases
        .into_iter()
        .map(|(name, input, params)| {
            // Snapshot the raw bytes *before* recolouring. The engine works in
            // place, so reading the buffer afterwards would record the result as
            // the input and every case would trivially "pass" against itself.
            let original = input.as_raw().clone();
            let mut buffer = input;
            let expected = wheel_engines::recolor::recolor_rgba_bytes(&mut buffer, &params);

            // A no-op case is the one exception: it exists to prove the
            // frontend leaves the buffer alone too. Everything else has to
            // actually change something, or the case proves nothing.
            if !params.is_noop() {
                assert_ne!(
                    original, expected,
                    "case {name} produced no change, so it cannot prove anything"
                );
            }

            json!({
                "name": name,
                "params": serde_json::to_value(&params).unwrap(),
                "input": original,
                "expected": expected,
            })
        })
        .collect();

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src/lib/__tests__/recolor-parity.json");

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    // Compact, not pretty-printed. The payload is raw RGBA, so `to_string_pretty`
    // puts one channel per line and turns a 60 KB fixture into 5,400 lines that
    // nobody reads and every diff of the file would touch. The case names and the
    // params are what a reader actually looks at, and those stay legible.
    std::fs::write(&path, serde_json::to_string(&fixture).unwrap()).unwrap();

    println!("wrote {} cases to {}", fixture.len(), path.display());
}

/// Cases chosen to cover the paths where a port could plausibly drift: the
/// tolerance boundary, nearest-rule-wins ordering, the two all-to-one modes, and
/// transparent pixels.
fn build_cases() -> Vec<(String, RgbaImage, RecolorParams)> {
    let mut cases = Vec::new();

    // A deterministic spread of colours. No RNG: a fixture that changes when the
    // test runs cannot prove anything.
    let palette = [
        Rgb::new(0x1E, 0x4F, 0xBF),
        Rgb::new(0x28, 0x76, 0xD2),
        Rgb::new(0x2C, 0x7D, 0xDB),
        Rgb::new(0xFD, 0xE0, 0x47),
        Rgb::new(0xB9, 0x1C, 0x1C),
        Rgb::new(0xE5, 0x3E, 0x4A),
        Rgb::new(0x8B, 0x45, 0x13),
        Rgb::new(0xFF, 0xFF, 0xFF),
        Rgb::new(0x00, 0x00, 0x00),
        Rgb::new(0x12, 0x34, 0x56),
    ];

    // Ramp from black to white plus the palette, so both the neutral axis and
    // real hues are covered.
    let mut ramp: Vec<Rgb> = (0..32u8)
        .map(|i| {
            let v = i * 8;
            Rgb::new(v, v, v)
        })
        .collect();
    ramp.extend(palette);

    let make_input = |pixels: &[Rgb]| {
        let mut image = RgbaImage::from_pixel(pixels.len() as u32, 1, Rgba([0, 0, 0, 255]));
        for (i, color) in pixels.iter().enumerate() {
            image.put_pixel(i as u32, 0, Rgba([color.r, color.g, color.b, 255]));
        }
        image
    };

    for tolerance in [0u8, 1, 10, 30, 50, 75, 100] {
        cases.push((
            format!("replace_tolerance_{tolerance}"),
            make_input(&ramp),
            RecolorParams {
                tolerance,
                rules: vec![
                    RecolorRule { from: palette[0], to: Rgb::new(0xB9, 0x1C, 0x1C) },
                    RecolorRule { from: palette[3], to: Rgb::new(0xCB, 0xE7, 0x1F) },
                ],
                ..Default::default()
            },
        ));
    }

    // Overlapping rules, so nearest-match has to win over list order.
    cases.push((
        "replace_overlapping_rules".into(),
        make_input(&ramp),
        RecolorParams {
            tolerance: 45,
            rules: vec![
                RecolorRule { from: palette[0], to: Rgb::new(0xFF, 0x00, 0x00) },
                RecolorRule { from: palette[1], to: Rgb::new(0x00, 0xFF, 0x00) },
                RecolorRule { from: palette[2], to: Rgb::new(0x00, 0x00, 0xFF) },
            ],
            ..Default::default()
        },
    ));

    // Duplicate `from` values: the first one has to win.
    cases.push((
        "replace_duplicate_sources".into(),
        make_input(&ramp),
        RecolorParams {
            tolerance: 30,
            rules: vec![
                RecolorRule { from: palette[0], to: Rgb::new(0xFF, 0x00, 0x00) },
                RecolorRule { from: palette[0], to: Rgb::new(0x00, 0xFF, 0x00) },
            ],
            ..Default::default()
        },
    ));

    // Alpha must survive untouched, and fully transparent pixels must be left
    // completely alone including their RGB.
    let mut alpha = RgbaImage::from_pixel(ramp.len() as u32 + 4, 1, Rgba([0, 0, 0, 255]));
    for (i, color) in ramp.iter().enumerate() {
        alpha.put_pixel(i as u32, 0, Rgba([color.r, color.g, color.b, 255]));
    }
    let base = ramp.len() as u32;
    alpha.put_pixel(base, 0, Rgba([palette[0].r, palette[0].g, palette[0].b, 77]));
    alpha.put_pixel(base + 1, 0, Rgba([palette[0].r, palette[0].g, palette[0].b, 0]));
    alpha.put_pixel(base + 2, 0, Rgba([0xFF, 0x00, 0x00, 0]));
    alpha.put_pixel(base + 3, 0, Rgba([palette[0].r, palette[0].g, palette[0].b, 255]));

    cases.push((
        "replace_with_alpha".into(),
        alpha.clone(),
        RecolorParams {
            tolerance: 30,
            rules: vec![RecolorRule { from: palette[0], to: Rgb::new(0xFF, 0xFF, 0xFF) }],
            ..Default::default()
        },
    ));
    cases.push((
        "all_to_one_with_alpha".into(),
        alpha,
        RecolorParams {
            mode: RecolorMode::AllToOne,
            all_to_one: Rgb::new(0xCB, 0xE7, 0x1F),
            preserve_shading: true,
            ..Default::default()
        },
    ));

    // Both all-to-one modes, over the same input, because reconstructing in
    // OKLab is where a port is most likely to round differently.
    for preserve in [true, false] {
        cases.push((
            format!("all_to_one_preserve_{preserve}"),
            make_input(&ramp),
            RecolorParams {
                mode: RecolorMode::AllToOne,
                all_to_one: Rgb::new(0xCB, 0xE7, 0x1F),
                preserve_shading: preserve,
                ..Default::default()
            },
        ));
    }

    // A saturated target, which pushes the OKLab reconstruction towards clipping
    // and would expose a difference in rounding or gamut handling.
    cases.push((
        "all_to_one_saturated_target".into(),
        make_input(&ramp),
        RecolorParams {
            mode: RecolorMode::AllToOne,
            all_to_one: Rgb::new(0x00, 0xFF, 0x7F),
            preserve_shading: true,
            ..Default::default()
        },
    ));

    // No rules at all: the buffer must come back byte-identical.
    cases.push((
        "replace_no_rules".into(),
        make_input(&ramp),
        RecolorParams::default(),
    ));

    cases
}
