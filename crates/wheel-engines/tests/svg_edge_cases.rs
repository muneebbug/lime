//! An adversarial corpus of SVG documents, run through both paths.
//!
//!   cargo test -p wheel-engines svg_edge_cases -- --ignored --nocapture
//!
//! Every case is a *valid* document that paints pixels, because the point is to
//! find engine gaps, not to hand resvg invalid XML it is right to refuse. The
//! wrapper supplies the root element and a viewBox; a case supplies paint.

use std::path::PathBuf;

use wheel_engines::recolor::{RecolorParams, RecolorRule, Rgb};
use wheel_engines::svg_recolor;

struct Case {
    name: &'static str,
    /// A complete document, when the case needs its own root.
    body: &'static str,
    /// Colours the document declares, as hex.
    declared: &'static [&'static str],
    /// Colour-looking text that is not a paint and must survive untouched.
    preserved: &'static [&'static str],
    /// True when the document is not valid XML, so a parse failure is correct.
    ///
    /// XML is case sensitive and SVG is an XML vocabulary with lowercase element
    /// names, so `<RECT>` is not an SVG. resvg is right to refuse it; the only
    /// thing worth checking is that we refuse it too rather than half-editing it.
    invalid_xml: bool,
    /// True when resvg does not render the construct at all.
    ///
    /// usvg has no CSS engine behind `var()`, and evaluates media queries against
    /// the viewport rather than the viewBox. Those documents declare a colour we
    /// can extract and rewrite correctly, but nothing is ever painted, so there is
    /// no pixel to find afterwards.
    renderer_blind: bool,
    /// The exact number of paints the list must report.
    ///
    /// Membership alone is not enough. The bug that started all this was a count
    /// that was wrong by a factor of fifteen while every colour it contained was
    /// legitimate, so the number is what gets pinned.
    paints: Option<usize>,
}

/// A full-bleed rect, so the document always paints something the raster check
/// can see.
const CASES: &[Case] = &[
    Case { name: "plain_hex", body: r##"<svg viewBox="0 0 100 100"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "uppercase_tags", body: r##"<SVG VIEWBOX="0 0 100 100"><RECT FILL="#FF5200" x="0" y="0" width="100" height="100"/></SVG>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: true, renderer_blind: false, paints: Some(1) },
    Case { name: "single_quoted_attributes", body: "<svg viewBox='0 0 100 100'><rect fill='#FF5200' x='0' y='0' width='100' height='100'/></svg>", declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "newlines_and_tabs_in_attributes", body: "<svg\n\tviewBox=\"0 0 100 100\"\n>\n<rect\n\tfill=\t\"#FF5200\"\n\tx=\"0\" y=\"0\" width=\"100\" height=\"100\"\n/>\n</svg>", declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "crlf_line_endings", body: "<svg viewBox=\"0 0 100 100\">\r\n<rect fill=\"#FF5200\" x=\"0\" y=\"0\" width=\"100\" height=\"100\"/>\r\n</svg>", declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "doctype_before_root", body: r##"<?xml version="1.0"?><!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"><svg viewBox="0 0 100 100"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },

    // Colour syntax the parser has to understand.
    Case { name: "short_hex", body: r##"<svg viewBox="0 0 100 100"><rect fill="#f50" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5500"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "alpha_hex_is_not_a_flat_paint", body: r##"<svg viewBox="0 0 100 100"><rect fill="#FF520080" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "hsl", body: r##"<svg viewBox="0 0 100 100"><rect fill="hsl(18,100%,32%)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#A33100"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "hsla", body: r##"<svg viewBox="0 0 100 100"><rect fill="hsla(18,100%,32%,1)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#A33100"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "rgb_percentages", body: r##"<svg viewBox="0 0 100 100"><rect fill="rgb(100%, 32.2%, 0%)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "rgba_floats", body: r##"<svg viewBox="0 0 100 100"><rect fill="rgba(255,82,0,1.0)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "named_colour", body: r##"<svg viewBox="0 0 100 100"><rect fill="coral" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF7F50"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "display_p3_twin_of_a_hex", body: r##"<svg viewBox="0 0 100 100"><rect fill="#FF5200" style="fill:color(display-p3 1.0000 0.3216 0.0000)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },

    // Style blocks, which is where declarations get complicated.
    Case { name: "style_element_class", body: r##"<svg viewBox="0 0 100 100"><style>.a{fill:#FF5200}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "two_style_elements", body: r##"<svg viewBox="0 0 100 100"><style>.a{fill:#FF5200}</style><style>.b{stroke:#2876D2}</style><rect fill="#2876D2" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200", "#2876D2"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(2) },
    Case { name: "media_query_in_style", body: r##"<svg viewBox="0 0 100 100"><style>@media (min-width:100px){.a{fill:#FF5200}}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: true, paints: Some(1) },
    Case { name: "style_attribute_trailing_semicolon", body: r##"<svg viewBox="0 0 100 100"><rect style="fill:#FF5200;stroke:#2876D2;" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200", "#2876D2"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(2) },
    Case { name: "style_attribute_important", body: r##"<svg viewBox="0 0 100 100"><rect style="fill:#FF5200 !important" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "style_element_important", body: r##"<svg viewBox="0 0 100 100"><style>.a{fill:#FF5200 !important}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "style_element_comment_inside", body: r##"<svg viewBox="0 0 100 100"><style>/* note */ .a{fill:#FF5200}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "css_var_indirection", body: r##"<svg viewBox="0 0 100 100"><style>:root{--brand:#FF5200}.a{fill:var(--brand)}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: true, paints: Some(1) },

    // Gradients and defs.
    Case { name: "linear_gradient_stops", body: r##"<svg viewBox="0 0 100 100"><defs><linearGradient id="g"><stop offset="0" stop-color="#FF5200"/><stop offset="1" stop-color="#2876D2"/></linearGradient></defs><rect fill="url(#g)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200", "#2876D2"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(2) },
    Case { name: "gradient_stop_opacity", body: r##"<svg viewBox="0 0 100 100"><defs><linearGradient id="g"><stop offset="0" stop-color="#FF5200" stop-opacity="0.5"/><stop offset="1" stop-color="#FF5200"/></linearGradient></defs><rect fill="url(#g)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },

    // Things that look like paint but are not.
    Case { name: "comment_holding_markup", body: r##"<svg viewBox="0 0 100 100"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/><!-- <rect fill="#2876D2"/> --></svg>"##, declared: &["#FF5200"], preserved: &["#2876D2"], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "cdata_script_holding_markup", body: r##"<svg viewBox="0 0 100 100"><script><![CDATA[ var s = "fill:#2876D2"; ]]></script><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &["#2876D2"], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "title_and_desc_prose", body: r##"<svg viewBox="0 0 100 100"><title>#FF5200 and #2876D2</title><desc>mentions fill:#2876D2</desc><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &["#2876D2"], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "fill_none", body: r##"<svg viewBox="0 0 100 100"><rect fill="none" stroke="#FF5200" stroke-width="10" x="5" y="5" width="90" height="90"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "current_color", body: r##"<svg viewBox="0 0 100 100"><rect fill="currentColor" color="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "path_data_with_hashlike_digits", body: r##"<svg viewBox="0 0 100 100"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/><path d="M10 10L11.5 12.5A3 3 0 0 1 14 14Z" fill="#2876D2"/></svg>"##, declared: &["#FF5200", "#2876D2"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(2) },
    Case { name: "xlink_use", body: r##"<svg viewBox="0 0 100 100" xmlns:xlink="http://www.w3.org/1999/xlink"><defs><rect id="r" x="0" y="0" width="100" height="100"/></defs><use xlink:href="#r" fill="#FF5200"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },

    // Transform shapes like the file that started this.
    Case { name: "non_integer_transform_scale", body: r##"<svg viewBox="0 0 100 100"><g transform="matrix(1.084041,0,0,1.084041,3.5,7.25)"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></g></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "two_opaque_shapes_touching", body: r##"<svg viewBox="0 0 100 100"><rect fill="#FFFEEE" x="0" y="0" width="100" height="100"/><circle cx="50" cy="50" r="40" fill="#201F1F"/></svg>"##, declared: &["#FFFEEE", "#201F1F"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(2) },

    // Degenerate documents.
    Case { name: "empty_root", body: r##"<svg viewBox="0 0 100 100"/>"##, declared: &[], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(0) },
    Case { name: "no_colours", body: r##"<svg viewBox="0 0 100 100"><path d="M0 0L100 100"/></svg>"##, declared: &[], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(0) },
    Case { name: "percentage_root_size", body: r##"<svg width="100%" height="100%" viewBox="0 0 100 100"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "long_desc_blob", body: r##"<svg viewBox="0 0 100 100"><desc>LOBBLOB</desc><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },

    // Constructs a colour can hide in that are not obvious paint properties.
    Case { name: "drop_shadow_colour", body: r##"<svg viewBox="0 0 100 100"><filter id="f"><feDropShadow dx="1" dy="1" stdDeviation="1" flood-color="#FF5200"/></filter><rect filter="url(#f)" fill="#2876D2" x="10" y="10" width="80" height="80"/></svg>"##, declared: &["#FF5200", "#2876D2"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(2) },
    Case { name: "mask_colour", body: r##"<svg viewBox="0 0 100 100"><mask id="m"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></mask><rect mask="url(#m)" fill="#2876D2" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200", "#2876D2"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(2) },
    Case { name: "filter_inside_a_style_block", body: r##"<svg viewBox="0 0 100 100"><style>.a{filter:drop-shadow(0 0 2px #FF5200);fill:#2876D2}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#2876D2"], preserved: &["#FF5200"], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "attribute_selector", body: r##"<svg viewBox="0 0 100 100"><style>[data-x]{fill:#FF5200}</style><rect data-x="1" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "compound_and_pseudo_class_selectors", body: r##"<svg viewBox="0 0 100 100"><style>.a.b:not(.c){fill:#FF5200}</style><rect class="a b" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "supports_at_rule", body: r##"<svg viewBox="0 0 100 100"><style>@supports (fill: red){.a{fill:#FF5200}}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: true, paints: Some(1) },
    Case { name: "font_face_block", body: r##"<svg viewBox="0 0 100 100"><style>@font-face{font-family:x;src:url(a.woff)}text{fill:#FF5200}</style><text x="0" y="50">hi</text></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "colour_after_a_url_paint", body: r##"<svg viewBox="0 0 100 100"><defs><linearGradient id="g"><stop stop-color="#2876D2"/></linearGradient></defs><rect fill="url(#g) #FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &["#2876D2"], invalid_xml: false, renderer_blind: false, paints: Some(2) },
    Case { name: "css_comment_inside_a_value", body: r##"<svg viewBox="0 0 100 100"><rect style="fill:/* brand */#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "css_comment_holding_a_separator", body: r##"<svg viewBox="0 0 100 100"><style>/* } */.a{fill:#FF5200}</style><rect class="a" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "nested_var_in_a_style_attribute", body: r##"<svg viewBox="0 0 100 100"><style>:root{--brand:#FF5200}</style><rect style="fill:var(--brand)" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: true, paints: Some(1) },

    // Must not be touched: a colour inside a URL is opaque payload, not paint.
    Case { name: "data_uri_in_href_is_not_a_paint", body: r##"<svg viewBox="0 0 100 100"><image href="data:image/svg+xml,%3Csvg%3E%3Crect%20fill%3D%22%23FF5200%22%2F%3E%3C%2Fsvg%3E" x="0" y="0" width="100" height="100"/><rect fill="#2876D2" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#2876D2"], preserved: &["%23FF5200"], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "fragment_identifier_is_not_a_paint", body: r##"<svg viewBox="0 0 100 100"><rect fill="#FF5200" x="0" y="0" width="100" height="100"/><use href="#FF5200"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },

    // Stress.
    Case { name: "deeply_nested_blocks", body: r##"<svg viewBox="0 0 100 100"><style>DEEPNEST</style><rect fill="#FF5200" x="0" y="0" width="100" height="100"/></svg>"##, declared: &["#FF5200"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(1) },
    Case { name: "many_repeats_of_few_paints", body: r##"<svg viewBox="0 0 100 100">MANYPRECTS</svg>"##, declared: &["#2876D2", "#FF7F50"], preserved: &[], invalid_xml: false, renderer_blind: false, paints: Some(18) },
];

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
/// Fill in the placeholders that keep the corpus readable.
fn expand(body: &str) -> String {
    let blob = "x".repeat(40_000);
    let deep = format!("@media a{{{}}}", "@media b{".repeat(200));
    // A palette whose members are far apart in OKLab, so the same-paint floor
    // does not fold them together. Repeating it exercises counting and speed
    // rather than the floor, which has its own tests.
    let palette = [
        "#FF0000", "#00FF00", "#0000FF", "#FFFF00", "#FF00FF", "#00FFFF",
        "#800000", "#008000", "#000080", "#808000", "#800080", "#008080",
        "#C0C0C0", "#FF7F50", "#2876D2", "#20B2AA", "#DDA0DD", "#F5DEB3",
    ];
    let many: String = (0..540)
        .map(|i| {
            let hex = palette[i % palette.len()];
            format!(r##"<rect fill="{hex}" x="0" y="0" width="1" height="1"/>"##)
        })
        .collect();

    body.replace("LOBBLOB", &blob)
        .replace("DEEPNEST", &deep)
        .replace("MANYPRECTS", &many)
}

#[test]
#[ignore = "diagnostic corpus; run deliberately"]
fn svg_edge_cases() {
    let dir = temp_dir("wheel_svg_edge_cases");
    let mut failures: Vec<String> = Vec::new();

    // The colour every declared paint is rewritten to. Checked exactly, not by
    // "is it dark", which an unpainted black pixel would also satisfy.
    let mark = Rgb::new(0x01, 0x02, 0x03);

    for case in CASES {
        let svg = expand(case.body);

        let path = dir.join(format!("{}.svg", case.name));
        std::fs::write(&path, &svg).unwrap();

        if case.invalid_xml {
            // Must be refused cleanly, not half-edited.
            if wheel_engines::image_convert::load_svg(&path).is_ok() {
                failures.push(format!("{}/open: invalid XML was accepted", case.name));
            }
            println!("{:<38} invalid xml, correctly refused", case.name);
            continue;
        }

        // 1. Declared colours.
        let paints = svg_recolor::distinct_paints(&svg);
        let found: Vec<String> = paints.iter().map(|p| p.color.to_hex()).collect();

        for expected in case.declared {
            let hex = Rgb::from_hex(expected).unwrap().to_hex();
            if !found.contains(&hex) {
                failures.push(format!("{}/list: expected {hex}, got {found:?}", case.name));
            }
        }

        if let Some(expected) = case.paints {
            if paints.len() != expected {
                failures.push(format!(
                    "{}/count: expected {expected} paints, got {}: {found:?}",
                    case.name,
                    paints.len()
                ));
            }
        }

        // 2. Rewrite every declared colour.
        let mut params = RecolorParams { tolerance: 0, ..Default::default() };
        for expected in case.declared {
            params.rules.push(RecolorRule {
                from: Rgb::from_hex(expected).unwrap(),
                to: mark,
            });
        }

        let out_path = dir.join(format!("{}-out.svg", case.name));
        match svg_recolor::recolor_svg_file(&path, &out_path, &params) {
            Ok(_) => {
                // The declared paints must be gone from the *paint list*. Checking
                // the raw text instead would fail on colour names mentioned in a
                // <title>, which are prose and must not be rewritten.
                let written = std::fs::read_to_string(&out_path).unwrap();
                let after: Vec<String> = svg_recolor::distinct_paints(&written)
                    .iter()
                    .map(|p| p.color.to_hex())
                    .collect();

                for expected in case.declared {
                    let hex = Rgb::from_hex(expected).unwrap().to_hex();
                    if after.contains(&hex) {
                        failures.push(format!("{}/save: {hex} still a paint: {after:?}", case.name));
                    }
                }

                // Text that merely looks like a colour must be byte-identical.
                for preserved in case.preserved {
                    if !written.contains(preserved) {
                        failures.push(format!(
                            "{}/save: {preserved} destroyed but is not a paint",
                            case.name
                        ));
                    }
                }
            }
            Err(e) => failures.push(format!("{}/save: {e:#}", case.name)),
        }

        // 3. Output must still be a valid document that renders the new colour.
        //
        // First confirm the *original* actually paints at least one declared
        // colour. If resvg ignores the construct - it has no CSS engine behind
        // `var()`, media queries are judged against the viewport, a gradient
        // endpoint lands on no pixel - then there is nothing to recolour and the
        // check would be testing the renderer rather than this tool.
        let declared_rgb: Vec<[u8; 3]> = case
            .declared
            .iter()
            .filter_map(|h| Rgb::from_hex(h))
            .map(|c| [c.r, c.g, c.b])
            .collect();

        let original_paints = wheel_engines::image_convert::load_svg(&path)
            .map(|img| {
                let rgba = img.to_rgba8();
                rgba.pixels()
                    .any(|p| p.0[3] > 200 && declared_rgb.contains(&[p.0[0], p.0[1], p.0[2]]))
            })
            .unwrap_or(false);

        if original_paints && case.renderer_blind {
            failures.push(format!(
                "{}/corpus: marked renderer-blind but resvg paints it",
                case.name
            ));
        }

        match wheel_engines::image_convert::load_svg(&out_path) {
            Ok(_) if case.declared.is_empty() || !original_paints => {}
            Ok(img) => {
                let rgba = img.to_rgba8();
                let want = [mark.r, mark.g, mark.b];
                let hit = rgba
                    .pixels()
                    .any(|p| p.0[3] > 200 && [p.0[0], p.0[1], p.0[2]] == want);
                if !hit {
                    failures.push(format!("{}/raster: {} never appears", case.name, mark.to_hex()));
                }
            }
            Err(e) => failures.push(format!("{}/raster: output no longer parses: {e:#}", case.name)),
        }

        // 4. The original must open.
        if let Err(e) = wheel_engines::image_convert::load_svg(&path) {
            failures.push(format!("{}/open: {e:#}", case.name));
        }

        let note = if original_paints { "" } else { "  (nothing painted to check)" };
        println!("{:<38} {found:?}{note}", case.name);
    }

    let _ = std::fs::remove_dir_all(&dir);

    if failures.is_empty() {
        println!("\nall {} cases passed", CASES.len());
    } else {
        println!("\n{} FAILURES out of {} cases:", failures.len(), CASES.len());
        for f in &failures {
            println!("  - {f}");
        }
    }
}
