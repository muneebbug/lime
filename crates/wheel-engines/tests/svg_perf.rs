//! Timing guards for the SVG paths.
//!
//!   cargo test -p wheel-engines svg_perf -- --ignored --nocapture
//!
//! Budgets are deliberately loose. They exist to catch an accidental quadratic
//! or an accidental full rasterisation, not to measure the machine.

use std::time::Instant;

use wheel_engines::recolor::{RecolorParams, RecolorRule, Rgb};
use wheel_engines::svg_recolor;

/// Debug builds run this code unoptimised, which costs roughly an order of
/// magnitude. The budgets exist to catch an accidental quadratic, so they are
/// scaled to the profile rather than tuned to one machine's noise.
const fn budgets() -> (u128, u128, u128) {
    if cfg!(debug_assertions) {
        (600, 1500, 3000)
    } else {
        (250, 400, 900)
    }
}

/// Colours far enough apart in OKLab that the same-paint floor keeps them
/// separate, which is what real artwork uses.
const PALETTE: &[&str] = &[
    "#FF0000", "#00FF00", "#0000FF", "#FFFF00", "#FF00FF", "#00FFFF",
    "#800000", "#008000", "#000080", "#808000", "#800080", "#008080",
    "#C0C0C0", "#FF7F50", "#2876D2", "#20B2AA", "#DDA0DD", "#F5DEB3",
];

/// A document with many literals, styles and rules.
fn big_svg(rects: usize) -> String {
    let palette = PALETTE;

    let mut svg = String::from(r##"<svg viewBox="0 0 1000 1000"><style>"##);
    for i in 0..200 {
        let color = palette[i % palette.len()];
        svg.push_str(&format!("@media (min-width:{i}px){{.r{i}{{fill:{color}}}}}"));
    }
    svg.push_str("</style>");
    for i in 0..rects {
        let color = palette[i % palette.len()];
        svg.push_str(&format!(
            r##"<rect class="r{}" fill="{color}" x="{}" y="{}" width="4" height="4"/>"##,
            i % 200,
            i % 250,
            i / 250
        ));
    }
    svg.push_str("</svg>");
    svg
}

#[test]
#[ignore = "timing guard; run deliberately"]
fn svg_perf() {
    let (budget_extract, budget_rewrite, budget_rasterise) = budgets();
    let dir = std::env::temp_dir().join("wheel_svg_perf");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let svg = big_svg(20_000);
    let path = dir.join("big.svg");
    std::fs::write(&path, &svg).unwrap();
    println!("document: {:.1} MB, 20000 rects", svg.len() as f64 / 1e6);

    let start = Instant::now();
    let paints = svg_recolor::distinct_paints(&svg);
    let extract = start.elapsed();
    println!(
        "distinct_paints: {:?} -> {} paints (budget {budget_extract} ms)",
        extract,
        paints.len()
    );
    assert!(
        extract.as_millis() < budget_extract,
        "listing a {}-paints document took {extract:?}",
        paints.len()
    );
    assert_eq!(paints.len(), PALETTE.len(), "the palette must not be split");

    // One rule per distinct paint, the way the window sends them.
    let params = RecolorParams {
        tolerance: 0,
        rules: paints
            .iter()
            .map(|p| RecolorRule {
                from: p.color,
                to: Rgb::new(0x01, 0x02, 0x03),
            })
            .collect(),
        ..Default::default()
    };

    let start = Instant::now();
    let out = dir.join("out.svg");
    let result = svg_recolor::recolor_svg_file(&path, &out, &params).unwrap();
    let rewrite = start.elapsed();
    println!(
        "rewrite {} literals: {:?} (budget {budget_rewrite} ms)",
        result.replaced, rewrite
    );
    assert!(
        rewrite.as_millis() < budget_rewrite,
        "rewriting {}-literals took {rewrite:?}",
        result.replaced
    );

    // Nothing of the original palette may survive. The replacement is itself a
    // paint, so the check is that *only* it is left.
    let mark = Rgb::new(0x01, 0x02, 0x03);
    let after = svg_recolor::distinct_paints(&std::fs::read_to_string(&out).unwrap());
    let survivors: Vec<String> = after.iter().map(|p| p.color.to_hex()).collect();
    assert_eq!(
        survivors,
        vec![mark.to_hex()],
        "expected only the replacement colour, found {survivors:?}"
    );

    let start = Instant::now();
    let image = wheel_engines::image_convert::load_svg(&out).unwrap();
    let raster = start.elapsed();
    println!(
        "rasterise: {:?} -> {}x{} (budget {budget_rasterise} ms)",
        raster,
        image.width(),
        image.height()
    );
    assert!(
        raster.as_millis() < budget_rasterise,
        "rasterising took {raster:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
