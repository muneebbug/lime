//! Recolour an SVG by editing its source, rather than by rasterising it.
//!
//! Why this exists: SVG is vector markup, so recolouring it through the pixel
//! pipeline throws away everything that makes it an SVG. The result is a
//! rasterised image with no paths, no scalability and nothing left to edit - and
//! asking to "save as SVG" produced a PNG with the wrong extension, because there
//! was no SVG encoder to hand.
//!
//! Editing the markup keeps the file what it was: still paths, still resolution
//! independent, still a few kilobytes instead of a raster.
//!
//! # How colours are found
//!
//! Attributes and CSS declarations are parsed, rather than the document being
//! scanned for anything shaped like a colour. The distinction matters:
//!
//! ```text
//! <linearGradient id="abc"><stop stop-color="red"/></linearGradient>
//! ```
//!
//! A substring search for `#abc` matches that fragment identifier. A parser
//! looking at an attribute value sees a colour. Scanning for colour-shaped text
//! and hoping is how a recolour tool quietly corrupts a file.
//!
//! # What is matched
//!
//! A colour literal is compared against the active rules in OKLab, using the same
//! tolerance as the raster path. Where a literal matched, it is rewritten as
//! `#RRGGBB` in place and the surrounding markup is preserved byte for byte. A
//! literal that matched nothing is left completely alone.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::info;

use crate::recolor::{
    ExtractedColor, Lab, RecolorMode, RecolorParams, Rgb, tolerance_to_distance,
};

/// A colour literal located in the source text.
#[derive(Debug, Clone, PartialEq)]
struct ColorSpan {
    /// Byte offsets into the original source, so the rewrite is a pure splice.
    start: usize,
    end: usize,
    color: Rgb,
}

/// Attributes whose value is a paint, and therefore worth parsing as a colour.
///
/// SVG's own paint attributes, plus the CSS properties that commonly appear in a
/// `style` attribute or a `<style>` block in real files. A colour in an unrelated
/// attribute - a `d` path, an `id` - is not something to recolour.
const PAINT_PROPERTIES: &[&str] = &[
    "fill",
    "stroke",
    "stop-color",
    "flood-color",
    "lighting-color",
    "color",
    "background",
    "background-color",
    "border-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "outline-color",
    "text-decoration-color",
    "caret-color",
    "column-rule-color",
];

/// Outcome of editing an SVG's source.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgRecolorResult {
    pub output_path: PathBuf,
    /// How many colour literals were rewritten.
    pub replaced: u64,
    /// How many colour literals the source declared.
    pub found: u64,
    /// True when nothing matched, so the file was written through untouched.
    pub unchanged: bool,
}

/// Recolour `input` by editing its markup, writing the result to `output`.
///
/// Reads and writes text, so the output is always SVG regardless of the source's
/// extension. Never rasterises.
pub fn recolor_svg_file(
    input: &Path,
    output: &Path,
    params: &RecolorParams,
) -> Result<SvgRecolorResult> {
    info!("Recolouring SVG source {:?} -> {:?}", input, output);

    let source = std::fs::read_to_string(input)
        .with_context(|| format!("Failed to read SVG source: {:?}", input))?;

    let found = count_colors(&source);
    let (edited, replaced) = recolor_svg_text(&source, params)?;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create destination directory: {:?}", parent)
            })?;
        }
    }

    std::fs::write(output, edited.as_bytes())
        .with_context(|| format!("Failed to write SVG to {:?}", output))?;

    Ok(SvgRecolorResult {
        output_path: output.to_path_buf(),
        replaced,
        found,
        unchanged: replaced == 0,
    })
}

/// Rewrite every matching colour literal in an SVG document.
///
/// Returns the new source and how many literals changed. Text outside the matched
/// spans is preserved exactly, including whitespace and quoting, so an edit that
/// changes nothing returns the file byte-identical.
pub fn recolor_svg_text(source: &str, params: &RecolorParams) -> Result<(String, u64)> {
    let spans = collect_colors(source.as_bytes());
    if spans.is_empty() {
        return Ok((source.to_string(), 0));
    }

    let rules = build_rules(params);
    let single = single_target(params);

    // Decide every replacement before touching the text, so nothing can be left
    // half-spliced.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for span in &spans {
        let replacement = match single {
            Some(target) => Some(target),
            None => nearest_rule(&rules, span.color, params),
        };
        if let Some(target) = replacement {
            edits.push((span.start, span.end, target.to_hex()));
        }
    }

    if edits.is_empty() {
        return Ok((source.to_string(), 0));
    }

    // Splicing back to front keeps the earlier offsets valid.
    let mut out = source.to_string();
    for (start, end, replacement) in edits.iter().rev() {
        out.replace_range(*start..*end, replacement);
    }

    Ok((out, edits.len() as u64))
}

/// How many colour literals a document declares.
pub fn count_colors(source: &str) -> u64 {
    collect_colors(source.as_bytes()).len() as u64
}

/// Every distinct colour a document declares, for the extracted-colours list.
pub fn declared_colors(source: &str) -> Vec<Rgb> {
    let mut seen = std::collections::HashSet::new();
    collect_colors(source.as_bytes())
        .into_iter()
        .map(|span| span.color)
        .filter(|color| seen.insert(*color))
        .collect()
}

/// How many literals use each colour, and how many distinct paints that is.
///
/// This is the count for an SVG, because it is the only one that describes the
/// artwork rather than the rasteriser's arithmetic.
///
/// Counting the rasterised pixels instead gives the wrong answer badly on vector
/// art. Antialiasing between two *opaque* shapes produces opaque intermediate
/// colours - a cream edge against a dark background is a blend, not a
/// translucent pixel - and a real 3-colour file with non-integer transforms
/// rasterised to 47 distinct colours, 44 of them two-hundred-pixel edge blends
/// that nobody chose.
///
/// So the list comes from the declared literals, and literals that are really one
/// paint are folded together using the same floor the save uses. That is what
/// makes a file declaring its colour as both a hex and a `color(display-p3 ...)`
/// twin report one colour rather than two.
pub fn distinct_paints(source: &str) -> Vec<ExtractedColor> {
    let spans = collect_colors(source.as_bytes());
    if spans.is_empty() {
        return Vec::new();
    }

    // Walk in source order so the representative of a group is the first literal
    // that introduced it, and the result does not depend on hashing.
    let mut groups: Vec<(Lab, Rgb, u64)> = Vec::new();
    let mut total_literals = 0u32;
    for span in spans {
        total_literals += 1;
        let lab = span.color.oklab();
        match groups
            .iter_mut()
            .find(|(group, _, _)| group.distance(lab) <= SAME_PAINT_FLOOR)
        {
            Some((_, _, count)) => *count += 1,
            None => groups.push((lab, span.color, 1)),
        }
    }

    let total = total_literals as f32;
    groups.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.to_hex().cmp(&b.1.to_hex())));
    groups
        .into_iter()
        .map(|(_, color, count)| ExtractedColor {
            color,
            count,
            coverage: count as f32 / total,
        })
        .collect()
}

/// The single colour for `AllToOne`, if that is the mode.
fn single_target(params: &RecolorParams) -> Option<Rgb> {
    match params.mode {
        RecolorMode::Replace => None,
        RecolorMode::AllToOne => Some(params.all_to_one),
    }
}

/// Pre-computed OKLab form of each rule, so the hot loop does no conversions.
///
/// Duplicate sources are dropped, matching the raster path: two rows pointing at
/// the same colour must not fight, and the first one wins.
fn build_rules(params: &RecolorParams) -> Vec<(Lab, Rgb)> {
    let mut seen = std::collections::HashSet::new();
    params
        .rules
        .iter()
        .filter(|rule| seen.insert(rule.from))
        .map(|rule| (rule.from.oklab(), rule.to))
        .collect()
}

/// Below this OKLab distance, two literals are treated as the same paint when
/// matching a rule.
///
/// This exists for one specific reason: an SVG can declare the same paint twice
/// in two notations that resolve a hair apart. A file written by a design tool
/// carries `fill="#FF5200"` in the attribute and `fill:color(display-p3 ...)` in
/// the style, and the style is the one a browser uses.
///
/// Measured on a real file: the two are 0.0304 apart. The nearest pair of
/// colours anyone would call genuinely different sat at 0.0388, so the floor is
/// set just above the notation gap.
///
/// That headroom is narrow, and deliberately so - the floor is only ever allowed
/// to grow the match radius, never shrink it, so the failure mode is recolouring
/// something a hair off rather than leaving part of the file untouched. The
/// swatch list does *not* use this number: colours are counted from the rendered
/// pixels, which is what the user is looking at, and needs no threshold at all.
const SAME_PAINT_FLOOR: f32 = 0.035;

/// The rule whose source colour is nearest within tolerance, or `None`.
///
/// Mirrors the raster path - nearest match wins, so the result never depends on
/// the order rules were added in - with one addition. A literal is also a match
/// when it is within `SAME_PAINT_FLOOR` of a rule, regardless of the slider,
/// because two notations of one paint have to travel together. Otherwise a rule
/// picked from the preview would leave the style attribute holding the original
/// colour, and the saved file would still render the old way while the preview
/// looked finished.
fn nearest_rule(rules: &[(Lab, Rgb)], color: Rgb, params: &RecolorParams) -> Option<Rgb> {
    if rules.is_empty() {
        return None;
    }

    let max_distance = tolerance_to_distance(params.tolerance).max(SAME_PAINT_FLOOR);
    let lab = color.oklab();

    let mut best: Option<(f32, Rgb)> = None;
    for (rule_lab, target) in rules {
        let distance = lab.distance(*rule_lab);
        if distance <= max_distance {
            match best {
                Some((current, _)) if current <= distance => {}
                _ => best = Some((distance, *target)),
            }
        }
    }

    best.map(|(_, target)| target)
}

// ---------------------------------------------------------------------------
// Locating colour literals
// ---------------------------------------------------------------------------

/// Find every colour literal the document declares.
fn collect_colors(bytes: &[u8]) -> Vec<ColorSpan> {
    let skipped = skipped_regions(bytes);

    // Custom properties first: `fill:var(--brand)` can only be resolved once
    // `--brand` is known, and a variable may be declared after its use.
    let mut variables = Vec::new();
    let mut spans = Vec::new();
    scan_document(bytes, &skipped, None, &mut variables, &mut spans);

    let mut resolved = Vec::new();
    scan_document(bytes, &skipped, Some(&variables), &mut Vec::new(), &mut resolved);
    spans.append(&mut resolved);

    spans.sort_by_key(|s| (s.start, s.end));
    spans.dedup_by_key(|s| s.start);
    spans
}

/// How deep `var()` indirection is followed before giving up.
const MAX_VAR_DEPTH: u8 = 8;

/// A custom property's declared value and where it sits in the document.
struct Variable {
    name: String,
    /// Offset of `value` within the document.
    offset: usize,
    value: Vec<u8>,
}

/// Run the attribute and style-block scans.
///
/// `variables` is `None` on the first pass, which only records custom property
/// declarations, and `Some` on the second, which resolves `var()` against them.
fn scan_document(
    bytes: &[u8],
    skipped: &[(usize, usize)],
    variables: Option<&[Variable]>,
    collected: &mut Vec<Variable>,
    spans: &mut Vec<ColorSpan>,
) {
    scan_attributes(bytes, skipped, variables, collected, spans);
    scan_style_blocks(bytes, variables, collected, spans);
}

/// Regions whose contents are not markup: comments, CDATA, processing
/// instructions and the doctype.
///
/// These look exactly like attributes to a scanner - `<!-- fill="#FF5200" -->`
/// parses as a perfectly good `fill` attribute - and recolouring inside a comment
/// would edit a file in a way nothing can see and nothing asked for.
fn skipped_regions(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let Some(open) = find_ci(bytes, b"<", cursor) else {
            break;
        };
        let rest = &bytes[open..];

        let marker: &[u8] = if rest.starts_with(b"<!--") {
            b"<!--"
        } else if rest.starts_with(b"<![CDATA[") {
            b"<![CDATA["
        } else if rest.starts_with(b"<?") {
            b"<?"
        } else if rest.starts_with(b"<!") {
            b"<!"
        } else {
            cursor = open + 1;
            continue;
        };

        let body_start = open + marker.len();
        let close: &[u8] = if marker == b"<!--" {
            b"-->"
        } else if marker == b"<![CDATA[" {
            b"]]>"
        } else if marker == b"<?" {
            b"?>"
        } else {
            b">"
        };

        match find_ci(bytes, close, body_start) {
            Some(end) => {
                regions.push((open, end + close.len()));
                cursor = end + close.len();
            }
            None => break,
        }
    }

    regions
}

/// Parse `name="value"` / `name='value'`, and test the value when `name` paints.
fn scan_attributes(
    bytes: &[u8],
    skipped: &[(usize, usize)],
    variables: Option<&[Variable]>,
    collected: &mut Vec<Variable>,
    spans: &mut Vec<ColorSpan>,
) {
    let mut i = 0;
    while i < bytes.len() {
        if !is_name_start(bytes[i]) || is_inside(skipped, i) {
            i += 1;
            continue;
        }

        let name_start = i;
        while i < bytes.len() && is_name_char(bytes[i]) {
            i += 1;
        }
        let name = String::from_utf8_lossy(&bytes[name_start..i]).to_ascii_lowercase();

        let mut j = i;
        while j < bytes.len() && is_space(bytes[j]) {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            continue;
        }
        j += 1;
        while j < bytes.len() && is_space(bytes[j]) {
            j += 1;
        }
        if j >= bytes.len() || (bytes[j] != b'"' && bytes[j] != b'\'') {
            i += 1;
            continue;
        }

        let quote = bytes[j];
        let value_start = j + 1;
        let mut k = value_start;
        while k < bytes.len() && bytes[k] != quote {
            k += 1;
        }
        if k >= bytes.len() {
            break;
        }
        let value = &bytes[value_start..k];
        i = k + 1;

        // A `style` attribute holds a declaration list, and is not itself a paint
        // property - so it is handled before the property check below.
        if name == "style" {
            scan_declaration_list(value, value_start, variables, collected, spans);
            continue;
        }

        if !PAINT_PROPERTIES.contains(&name.as_str()) {
            continue;
        }

        // A paint attribute can still hold a list, in the mixed case an authoring
        // tool emits.
        if looks_like_declaration_list(value) {
            scan_declaration_list(value, value_start, variables, collected, spans);
        } else {
            record_paint(value, value_start, variables, spans);
        }
    }
}

/// True for something shaped like `a:b;c:d`.
fn looks_like_declaration_list(value: &[u8]) -> bool {
    value.contains(&b':') && value.contains(&b';')
}

/// Find `prop: value` pairs inside a declaration list.
///
/// Values end at `;` *or* `}`. The brace matters for a `<style>` block, where
/// declarations close with `}` rather than a semicolon; without it the first
/// rule in the stylesheet swallowed the rest of the file as one value and found
/// no colour at all.
///
/// Parentheses and blocks are also stepped over rather than read as properties.
/// `@media (min-width:100px){.a{fill:#FF5200}}` is the case that forces this:
/// `min-width:100px)` looks exactly like a declaration, so a flat reader treats
/// everything after it as that property's value and the colour is never reached.
fn scan_declaration_list(
    value: &[u8],
    offset: usize,
    variables: Option<&[Variable]>,
    collected: &mut Vec<Variable>,
    spans: &mut Vec<ColorSpan>,
) {
    let mut i = 0;
    while i < value.len() {
        if let Some(next) = step_structure(value, offset, i, variables, collected, spans) {
            i = next;
            continue;
        }

        let name_start = i;
        // Not `is_name_char`: a colon separates the property from its value, so
        // treating it as a name character read `fill:#FF5200` as one long
        // property name and found no colour at all.
        while i < value.len() && is_declaration_name_char(value[i]) {
            i += 1;
        }
        let name = String::from_utf8_lossy(&value[name_start..i]).to_ascii_lowercase();

        while i < value.len() && is_space(value[i]) {
            i += 1;
        }
        if i >= value.len() {
            break;
        }

        // `media (min-width:100px)` and `.a{` both put a structure character
        // straight after a name, and both used to be read as a declaration whose
        // value ran to the end of the block.
        if let Some(next) = step_structure(value, offset, i, variables, collected, spans) {
            i = next;
            continue;
        }

        if value[i] != b':' {
            i += 1;
            continue;
        }
        i += 1;
        while i < value.len() && is_space(value[i]) {
            i += 1;
        }

        let value_start = i;
        while i < value.len() && !matches!(value[i], b';' | b'}' | b'{') {
            // A `url(data:...)` or `calc(...)` value may contain the separators
            // that would otherwise end it early, and so may a comment holding
            // one: `fill:/* ; */#FF5200` must not end at the semicolon.
            if value[i] == b'(' {
                i = match_paren(value, i);
                continue;
            }
            if starts_with_ci(&value[i..], b"/*") {
                match find_ci(&value[i..], b"*/", 2) {
                    Some(end) => i += end + 2,
                    None => break,
                }
                continue;
            }
            i += 1;
        }
        let value_end = i;

        // What was just read was a selector, not a declaration. `:root{`,
        // `.a{`, `a:hover{` and `@media ...{` all land here, and their value
        // swallowed the block that follows. Leave the brace for the next step so
        // the block's own declarations get scanned.
        if value_end < value.len() && value[value_end] == b'{' {
            continue;
        }

        while i < value.len() && matches!(value[i], b';' | b'}') {
            i += 1;
        }

        let raw = &value[value_start..value_end];
        let text = trim(raw);

        // A custom property is not itself a paint, but the colour it holds is:
        // `--brand:#FF5200` is what `fill:var(--brand)` paints with, so it has to
        // be listed and rewritten, or recolouring does nothing visible.
        if let Some(prop) = name.strip_prefix("--") {
            if !prop.is_empty() {
                collected.push(Variable {
                    name: prop.to_string(),
                    offset: offset + value_start,
                    value: text.to_vec(),
                });
                record_paint(text, offset + value_start, variables, spans);
            }
            continue;
        }

        if !PAINT_PROPERTIES.contains(&name.as_str()) {
            continue;
        }

        // A shorthand like `background: url(x) red` puts the colour after other
        // values. `record_paint` handles that by taking whatever follows the
        // `url(...)`, and `parse_css_color` insists on consuming the whole of
        // what it is given, so no extra length check is needed here. Adding one
        // made `fill: #FF5200 ` - a value with a space before the semicolon,
        // which is how every formatter writes it - look like a shorthand and be
        // skipped entirely.
        record_paint(text, offset + value_start, variables, spans);
    }
}

/// Step over one structural character, scanning a nested block if there is one.
///
/// Returns where to continue from, or `None` when `value[at]` is not structural.
fn step_structure(
    value: &[u8],
    offset: usize,
    at: usize,
    variables: Option<&[Variable]>,
    collected: &mut Vec<Variable>,
    spans: &mut Vec<ColorSpan>,
) -> Option<usize> {
    match value.get(at)? {
        // A media condition, or a function call inside a value.
        b'(' => Some(match_paren(value, at)),
        // CSS nests, so a block carries its own declarations.
        b'{' => {
            let inner_start = at + 1;
            match match_brace(value, at) {
                Some(inner_end) => {
                    scan_declaration_list(
                        &value[inner_start..inner_end],
                        offset + inner_start,
                        variables,
                        collected,
                        spans,
                    );
                    Some(inner_end + 1)
                }
                // Unterminated: take the rest of the input as the block.
                None => {
                    scan_declaration_list(
                        &value[inner_start..],
                        offset + inner_start,
                        variables,
                        collected,
                        spans,
                    );
                    Some(value.len())
                }
            }
        }
        b'}' | b';' => Some(at + 1),
        _ => None,
    }
}

/// Offset just past the `)` that closes the `(` at `open`.
fn match_paren(value: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < value.len() {
        if starts_with_ci(&value[i..], b"/*") {
            i += match find_ci(&value[i..], b"*/", 2) {
                Some(end) => end + 2,
                None => return value.len(),
            };
            continue;
        }
        match value[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            b'"' | b'\'' => {
                i = skip_quoted(value, i);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    value.len()
}

/// Offset just past the `}` that closes the `{` at `open`.
fn match_brace(value: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < value.len() {
        if starts_with_ci(&value[i..], b"/*") {
            i += match find_ci(&value[i..], b"*/", 2) {
                Some(end) => end + 2,
                None => return None,
            };
            continue;
        }
        match value[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'"' | b'\'' => {
                i = skip_quoted(value, i);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Offset just past the closing quote of the string starting at `open`.
fn skip_quoted(value: &[u8], open: usize) -> usize {
    let quote = value[open];
    let mut i = open + 1;
    while i < value.len() && value[i] != quote {
        if value[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    i.min(value.len().saturating_sub(1))
}

/// Record a paint property's value, following `var()` when it is one.
///
/// `var(--brand)` paints with whatever `--brand` holds, and the span recorded is
/// the *definition*, not the use. Rewriting the definition recolours every
/// element that references it in one edit, which is both what the artist meant
/// and the only way the rule can see the colour at all.
fn record_paint(
    text: &[u8],
    offset: usize,
    variables: Option<&[Variable]>,
    spans: &mut Vec<ColorSpan>,
) {
    let text = trim(strip_important(text));
    // `fill:/* brand */#FF5200` is the same declaration as `fill:#FF5200`.
    let comment = skip_leading_comments(text);
    let text = &text[comment..];
    let offset = offset + comment;

    if starts_with_ci(text, b"var(") {
        if let Some(vars) = variables {
            resolve_var(text, offset, vars, 0, spans);
        }
        return;
    }

    push_if_color(text, offset, spans);
}

/// Skip CSS comments and the whitespace after them at the start of a value,
/// returning how many bytes that took so the offset can follow.
fn skip_leading_comments(text: &[u8]) -> usize {
    let mut i = 0;
    loop {
        while i < text.len() && is_space(text[i]) {
            i += 1;
        }
        if !starts_with_ci(&text[i..], b"/*") {
            return i;
        }
        // An unterminated comment runs to the end, which is what a browser does
        // with the rest of the declaration rather than recovering mid-way.
        match find_ci(&text[i..], b"*/", 2) {
            Some(end) => i += end + 2,
            None => return text.len(),
        }
    }
}

/// Follow `var(--name)`, `var(--name, fallback)` and chains of either.
fn resolve_var(
    text: &[u8],
    text_offset: usize,
    variables: &[Variable],
    depth: u8,
    spans: &mut Vec<ColorSpan>,
) {
    if depth >= MAX_VAR_DEPTH {
        return;
    }

    let inner = match strip_call(text, b"var(") {
        Some(inner) => inner,
        None => return,
    };

    let comma = inner.value.iter().position(|b| *b == b',');
    let (name_part, fallback) = match comma {
        Some(at) => {
            let after = &inner.value[at + 1..];
            // The fallback's offset has to skip the space after the comma, or the
            // replacement is written over the whitespace and clips the last
            // character of the colour.
            let spaces = after.iter().take_while(|b| is_space(**b)).count();
            (
                &inner.value[..at],
                Some((inner.offset + at + 1 + spaces, trim(after))),
            )
        }
        None => (inner.value, None),
    };
    let name = String::from_utf8_lossy(name_part).trim().to_ascii_lowercase();

    match variables.iter().find(|v| v.name == name) {
        Some(var) => {
            let value = trim(strip_important(&var.value));
            if starts_with_ci(value, b"var(") {
                resolve_var(value, var.offset, variables, depth + 1, spans);
            } else {
                record_paint(value, var.offset, Some(variables), spans);
            }
        }
        // Undefined: the fallback, if any, is what renders.
        None => {
            // The offset has to come from inside the call, not from the start of
            // the value. Using the start of `var(` wrote the replacement over the
            // text of the reference itself.
            if let Some((offset, fallback)) = fallback {
                record_paint(fallback, text_offset + offset, Some(variables), spans);
            }
        }
    }
}

/// The contents of a `name(...)` call, and where those contents start within
/// `text`, plus the offset of the opening parenthesis.
fn strip_call<'a>(text: &'a [u8], call: &[u8]) -> Option<Inner<'a>> {
    if !starts_with_ci(text, call) {
        return None;
    }
    let open = call.len() - 1;
    let close = match_paren(text, open);
    if close > text.len() || !text[..close].ends_with(b")") {
        return None;
    }
    Some(Inner {
        offset: open + 1,
        value: &text[open + 1..close - 1],
    })
}

/// A slice and its position, so a nested offset can be resolved.
struct Inner<'a> {
    offset: usize,
    value: &'a [u8],
}

/// Drop a trailing `!important`, which is a modifier rather than part of the
/// value. `fill:#FF5200 !important` is the same paint as `fill:#FF5200`.
fn strip_important(text: &[u8]) -> &[u8] {
    const TAG: &[u8] = b"!important";

    let text = trim(text);
    let Some(at) = text.len().checked_sub(TAG.len()) else {
        return text;
    };
    if !text[at..].eq_ignore_ascii_case(TAG) {
        return text;
    }

    let head = trim(&text[..at]);
    // Only strip when what remains is a value in its own right. `!important` on
    // its own is not a colour, and a declaration whose value really is the text
    // `!important` should be left alone rather than turned into an empty value.
    if head.is_empty() || parse_css_color(head).is_err() {
        return text;
    }
    head
}

fn starts_with_ci(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len()
        && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

/// True when `offset` falls inside one of the sorted, non-overlapping regions.
fn is_inside(regions: &[(usize, usize)], offset: usize) -> bool {
    regions
        .binary_search_by(|(start, end)| {
            if offset < *start {
                std::cmp::Ordering::Greater
            } else if offset >= *end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// The contents of `<style>...</style>`, which hold the same declarations.
fn scan_style_blocks(
    bytes: &[u8],
    variables: Option<&[Variable]>,
    collected: &mut Vec<Variable>,
    spans: &mut Vec<ColorSpan>,
) {
    let mut cursor = 0;
    while let Some(found) = find_ci(bytes, b"<style", cursor) {
        let open_end = match find_ci(bytes, b">", found) {
            Some(end) => end + 1,
            None => break,
        };
        let close = match find_ci(bytes, b"</style", open_end) {
            Some(close) => close,
            None => break,
        };

        scan_declaration_list(
            &bytes[open_end..close],
            open_end,
            variables,
            collected,
            spans,
        );
        cursor = close + 7;
    }
}

/// Record a colour if `raw` parses as one.
fn push_if_color(raw: &[u8], start: usize, spans: &mut Vec<ColorSpan>) {
    let (inner_start, text) = match split_url(raw) {
        Some(found) => found,
        None => (0, raw),
    };

    let Ok(color) = parse_css_color(text) else {
        return;
    };

    spans.push(ColorSpan {
        start: start + inner_start,
        end: start + inner_start + text.len(),
        color,
    });
}

/// `url(#gradient)` is a reference, not a colour. Returns the offset within `raw`
/// and whatever follows the `url(...)`, or `None` if there is no `url`.
fn split_url(raw: &[u8]) -> Option<(usize, &[u8])> {
    let mut i = 0;
    while i + 3 < raw.len() {
        if raw[i..].len() >= 4 && raw[i..i + 4].eq_ignore_ascii_case(b"url(") {
            let mut j = i + 4;
            while j < raw.len() && raw[j] != b')' {
                j += 1;
            }
            return if j < raw.len() {
                Some((j + 1, trim(&raw[j + 1..])))
            } else {
                None
            };
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Parsing colour literals
// ---------------------------------------------------------------------------

/// Parse one CSS/SVG colour literal.
///
/// Hand written rather than a full CSS Color 4 parser: that would be more correct
/// and much more code, and the forms that actually appear in image files are a
/// short list. Unsupported spellings are rejected, which leaves them untouched
/// rather than guessing.
pub fn parse_css_color(text: &[u8]) -> Result<Rgb, ()> {
    let text = trim(text);
    if text.is_empty() {
        return Err(());
    }

    let lower = String::from_utf8_lossy(text).to_ascii_lowercase();

    // Hex, by a wide margin the most common.
    if lower.starts_with('#') {
        return parse_hex(&lower);
    }

    if let Some(open) = lower.find('(') {
        let name = lower[..open].trim();
        let close = lower.rfind(')').unwrap_or(lower.len());
        let args = &lower[open + 1..close];
        return match name {
            "rgb" | "rgba" => parse_rgb_args(args),
            "hsl" | "hsla" => parse_hsl_args(args),
            "color" => parse_color_function(args),
            _ => Err(()),
        };
    }

    named_color(&lower).ok_or(())
}

fn parse_hex(text: &str) -> Result<Rgb, ()> {
    let digits = &text[1..];

    match digits.len() {
        // Short form: each digit is doubled, so #abc is #aabbcc.
        3 | 4 => {
            let digit = |c: char| c.to_digit(16).map(|d| (d * 17) as u8).ok_or(());
            let mut chars = digits.chars();
            Ok(Rgb::new(
                digit(chars.next().ok_or(())?)?,
                digit(chars.next().ok_or(())?)?,
                digit(chars.next().ok_or(())?)?,
            ))
        }
        // Long form. An 8-digit value carries alpha, which is opacity rather than
        // colour, so only the first three pairs are the colour.
        6 | 8 => {
            if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(());
            }
            Ok(Rgb::new(
                u8::from_str_radix(&digits[0..2], 16).map_err(|_| ())?,
                u8::from_str_radix(&digits[2..4], 16).map_err(|_| ())?,
                u8::from_str_radix(&digits[4..6], 16).map_err(|_| ())?,
            ))
        }
        _ => Err(()),
    }
}

/// `rgb(255, 82, 0)` and `rgb(100%, 32%, 0%)`.
fn parse_rgb_args(args: &str) -> Result<Rgb, ()> {
    let parts = split_args(args);
    if parts.len() < 3 {
        return Err(());
    }

    let channel = |text: &str| -> Result<u8, ()> {
        if let Some(percent) = text.strip_suffix('%') {
            let value: f64 = percent.parse().map_err(|_| ())?;
            Ok((value / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8)
        } else {
            let value: f64 = text.parse().map_err(|_| ())?;
            Ok(value.round().clamp(0.0, 255.0) as u8)
        }
    };

    Ok(Rgb::new(
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?,
    ))
}

/// `hsl(18, 100%, 50%)` and the space-separated form CSS pickers emit.
fn parse_hsl_args(args: &str) -> Result<Rgb, ()> {
    let parts = split_args(args);
    if parts.len() < 3 {
        return Err(());
    }

    let hue_text = parts[0]
        .trim_end_matches("deg")
        .trim_end_matches("grad")
        .trim_end_matches("rad")
        .trim_end_matches("turn");
    let hue: f64 = hue_text.parse().map_err(|_| ())?;
    let saturation: f64 = parts[1].trim_end_matches('%').parse().map_err(|_| ())?;
    let lightness: f64 = parts[2].trim_end_matches('%').parse().map_err(|_| ())?;

    Ok(hsl_to_rgb(hue, saturation / 100.0, lightness / 100.0))
}

fn split_args(args: &str) -> Vec<&str> {
    args.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect()
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let h = h.rem_euclid(360.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let to_byte = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb::new(to_byte(r), to_byte(g), to_byte(b))
}

/// `color(display-p3 1 0.32 0)` and friends.
///
/// Wide-gamut spaces are converted to sRGB rather than rejected. Design tools
/// export them constantly - the file that prompted this module carried
/// `fill:color(display-p3 1.0000 0.3216 0.0000)` in its inline style - and
/// refusing them would leave half a document unrecoloured. Clipping to the sRGB
/// gamut is the correct conversion, and matches what a browser displays.
fn parse_color_function(args: &str) -> Result<Rgb, ()> {
    let mut parts = args.split_whitespace();
    let space = parts.next().unwrap_or("");

    match space {
        "srgb" | "display-p3" | "rec2020" | "a98-rgb" | "prophoto-rgb" => {
            let values: Vec<&str> = parts.take(3).collect();
            if values.len() < 3 {
                return Err(());
            }

            let component = |text: &str| -> Result<f64, ()> {
                // CSS allows both `50%` and `0.5` here.
                match text.strip_suffix('%') {
                    Some(percent) => percent.parse::<f64>().map_err(|_| ()).map(|v| v / 100.0),
                    None => text.parse::<f64>().map_err(|_| ()),
                }
            };

            let raw = [
                component(values[0])?.clamp(0.0, 1.0),
                component(values[1])?.clamp(0.0, 1.0),
                component(values[2])?.clamp(0.0, 1.0),
            ];

            // Display P3 uses the sRGB transfer function, so the values have to
            // be linearised before the matrix. Skipping that step treats the
            // encoded numbers as if they were linear, which lightens the whole
            // conversion badly - a mid orange came out as yellow.
            let linear = if space == "display-p3" {
                p3_to_srgb_linear(srgb_to_linear(raw))
            } else {
                // The remaining spaces are close enough to sRGB that treating
                // their primaries as sRGB costs a fraction of a step, and this is
                // a recolour rather than a colour-management tool.
                srgb_to_linear(raw)
            };

            Ok(Rgb::new(
                linear_to_byte(linear[0]),
                linear_to_byte(linear[1]),
                linear_to_byte(linear[2]),
            ))
        }
        // `currentColor`, `inherit`, `transparent`, `var(--x)`: all indirect.
        _ => Err(()),
    }
}

fn srgb_to_linear(c: [f64; 3]) -> [f64; 3] {
    c.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    })
}

fn linear_to_byte(v: f64) -> u8 {
    let c = v.clamp(0.0, 1.0);
    let encoded = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// Display P3 linear to sRGB linear, by the standard 3x3 matrix.
///
/// Written out rather than pulled in as a dependency: it is nine numbers, and
/// this crate has no colour-management dependency to borrow them from.
fn p3_to_srgb_linear(c: [f64; 3]) -> [f64; 3] {
    let m = [
        [1.2249401762805598, -0.22494041254070308, 0.0],
        [-0.04205695470968820, 1.0420810062583212, 0.0],
        [-0.01963755459033440, -0.07863604555063180, 1.0982736001409642],
    ];

    [
        m[0][0] * c[0] + m[0][1] * c[1] + m[0][2] * c[2],
        m[1][0] * c[0] + m[1][1] * c[1] + m[1][2] * c[2],
        m[2][0] * c[0] + m[2][1] * c[1] + m[2][2] * c[2],
    ]
}

/// The CSS named colours.
///
/// All of them, because a file using `fill="tomato"` has to recolour as reliably
/// as one using a hex literal, and a partial table would silently skip whatever
/// was missing from it.
fn named_color(name: &str) -> Option<Rgb> {
    let rgb = match name {
        "aliceblue" => (240, 248, 255),
        "antiquewhite" => (250, 235, 215),
        "aqua" | "cyan" => (0, 255, 255),
        "aquamarine" => (127, 255, 212),
        "azure" => (240, 255, 255),
        "beige" => (245, 245, 220),
        "bisque" => (255, 228, 196),
        "black" => (0, 0, 0),
        "blanchedalmond" => (255, 235, 205),
        "blue" => (0, 0, 255),
        "blueviolet" => (138, 43, 226),
        "brown" => (165, 42, 42),
        "burlywood" => (222, 184, 135),
        "cadetblue" => (95, 158, 160),
        "chartreuse" => (127, 255, 0),
        "chocolate" => (210, 105, 30),
        "coral" => (255, 127, 80),
        "cornflowerblue" => (100, 149, 237),
        "cornsilk" => (255, 248, 220),
        "crimson" => (220, 20, 60),
        "darkblue" => (0, 0, 139),
        "darkcyan" => (0, 139, 139),
        "darkgoldenrod" => (184, 134, 11),
        "darkgray" | "darkgrey" => (169, 169, 169),
        "darkgreen" => (0, 100, 0),
        "darkkhaki" => (189, 183, 107),
        "darkmagenta" => (139, 0, 139),
        "darkolivegreen" => (85, 107, 47),
        "darkorange" => (255, 140, 0),
        "darkorchid" => (153, 50, 204),
        "darkred" => (139, 0, 0),
        "darksalmon" => (233, 150, 122),
        "darkseagreen" => (143, 188, 143),
        "darkslateblue" => (72, 61, 139),
        "darkslategray" | "darkslategrey" => (47, 79, 79),
        "darkturquoise" => (0, 206, 209),
        "darkviolet" => (148, 0, 211),
        "deeppink" => (255, 20, 147),
        "deepskyblue" => (0, 191, 255),
        "dimgray" | "dimgrey" => (105, 105, 105),
        "dodgerblue" => (30, 144, 255),
        "firebrick" => (178, 34, 34),
        "floralwhite" => (255, 250, 240),
        "forestgreen" => (34, 139, 34),
        "fuchsia" => (255, 0, 255),
        "gainsboro" => (220, 220, 220),
        "ghostwhite" => (248, 248, 255),
        "gold" => (255, 215, 0),
        "goldenrod" => (218, 165, 32),
        "gray" | "grey" => (128, 128, 128),
        "green" => (0, 128, 0),
        "greenyellow" => (173, 255, 47),
        "honeydew" => (240, 255, 240),
        "magenta" => (255, 0, 255),
        "hotpink" => (255, 105, 180),
        "indianred" => (205, 92, 92),
        "indigo" => (75, 0, 130),
        "ivory" => (255, 255, 240),
        "khaki" => (240, 230, 140),
        "lavender" => (230, 230, 250),
        "lavenderblush" => (255, 240, 245),
        "lawngreen" => (124, 252, 0),
        "lemonchiffon" => (255, 250, 205),
        "lightblue" => (173, 216, 230),
        "lightcoral" => (240, 128, 128),
        "lightcyan" => (224, 255, 255),
        "lightgoldenrodyellow" => (250, 250, 210),
        "lightgray" | "lightgrey" => (211, 211, 211),
        "lightgreen" => (144, 238, 144),
        "lightpink" => (255, 182, 193),
        "lightsalmon" => (255, 160, 122),
        "lightseagreen" => (32, 178, 170),
        "lightskyblue" => (135, 206, 250),
        "lightslategray" | "lightslategrey" => (119, 136, 153),
        "lightsteelblue" => (176, 196, 222),
        "lightyellow" => (255, 255, 224),
        "lime" => (0, 255, 0),
        "limegreen" => (50, 205, 50),
        "linen" => (250, 240, 230),
        "maroon" => (128, 0, 0),
        "mediumaquamarine" => (102, 205, 170),
        "mediumblue" => (0, 0, 205),
        "mediumorchid" => (186, 85, 211),
        "mediumpurple" => (147, 112, 219),
        "mediumseagreen" => (60, 179, 113),
        "mediumslateblue" => (123, 104, 238),
        "mediumspringgreen" => (0, 250, 154),
        "mediumturquoise" => (72, 209, 204),
        "mediumvioletred" => (199, 21, 133),
        "midnightblue" => (25, 25, 112),
        "mintcream" => (245, 255, 250),
        "mistyrose" => (255, 228, 225),
        "moccasin" => (255, 228, 181),
        "navajowhite" => (255, 222, 173),
        "navy" => (0, 0, 128),
        "oldlace" => (253, 245, 230),
        "olive" => (128, 128, 0),
        "olivedrab" => (107, 142, 35),
        "orange" => (255, 165, 0),
        "orangered" => (255, 69, 0),
        "orchid" => (218, 112, 214),
        "palegoldenrod" => (238, 232, 170),
        "palegreen" => (152, 251, 152),
        "paleturquoise" => (175, 238, 238),
        "palevioletred" => (219, 112, 147),
        "papayawhip" => (255, 239, 213),
        "peachpuff" => (255, 218, 185),
        "peru" => (205, 133, 63),
        "pink" => (255, 192, 203),
        "plum" => (221, 160, 221),
        "powderblue" => (176, 224, 230),
        "purple" => (128, 0, 128),
        "rebeccapurple" => (102, 51, 153),
        "red" => (255, 0, 0),
        "rosybrown" => (188, 143, 143),
        "royalblue" => (65, 105, 225),
        "saddlebrown" => (139, 69, 19),
        "salmon" => (250, 128, 114),
        "sandybrown" => (244, 164, 96),
        "seagreen" => (46, 139, 87),
        "seashell" => (255, 245, 238),
        "sienna" => (160, 82, 45),
        "silver" => (192, 192, 192),
        "skyblue" => (135, 206, 235),
        "slateblue" => (106, 90, 205),
        "slategray" | "slategrey" => (112, 128, 144),
        "snow" => (255, 250, 250),
        "springgreen" => (0, 255, 127),
        "steelblue" => (70, 130, 180),
        "tan" => (210, 180, 140),
        "teal" => (0, 128, 128),
        "thistle" => (216, 191, 216),
        "tomato" => (255, 99, 71),
        "turquoise" => (64, 224, 208),
        "violet" => (238, 130, 238),
        "wheat" => (245, 222, 179),
        "white" => (255, 255, 255),
        "whitesmoke" => (245, 245, 245),
        "yellow" => (255, 255, 0),
        "yellowgreen" => (154, 205, 50),
        _ => return None,
    };

    Some(Rgb::new(rgb.0, rgb.1, rgb.2))
}

// ---------------------------------------------------------------------------
// Small byte helpers
// ---------------------------------------------------------------------------

fn trim(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && is_space(bytes[start]) {
        start += 1;
    }
    while end > start && is_space(bytes[end - 1]) {
        end -= 1;
    }
    &bytes[start..end]
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b':'
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':')
}

/// A CSS property name: like an attribute name, but without the colon that ends
/// it, and without the `;` or `}` that close a declaration.
fn is_declaration_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')
}

/// Case-insensitive search for a byte needle.
fn find_ci(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() {
        return None;
    }
    (from..=haystack.len().saturating_sub(needle.len()))
        .find(|&i| haystack[i..i + needle.len()].eq_ignore_ascii_case(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recolor::RecolorRule;

    fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb::new(r, g, b)
    }

    fn params_from(rules: &[(&str, &str)], tolerance: u8) -> RecolorParams {
        RecolorParams {
            rules: rules
                .iter()
                .map(|(from, to)| RecolorRule {
                    from: Rgb::from_hex(from).unwrap(),
                    to: Rgb::from_hex(to).unwrap(),
                })
                .collect(),
            tolerance,
            ..Default::default()
        }
    }

    fn recolor_to(svg: &str, from: &str, to: &str) -> (String, u64) {
        recolor_svg_text(svg, &params_from(&[(from, to)], 30)).unwrap()
    }

    // ---- colour parsing --------------------------------------------------

    #[test]
    fn parses_the_hex_forms() {
        assert_eq!(parse_css_color(b"#FF5200"), Ok(rgb(0xFF, 0x52, 0x00)));
        assert_eq!(parse_css_color(b"#ff5200"), Ok(rgb(0xFF, 0x52, 0x00)));
        assert_eq!(parse_css_color(b"#f50"), Ok(rgb(0xFF, 0x55, 0x00)));
        // Alpha suffixes are opacity, not colour.
        assert_eq!(parse_css_color(b"#FF520080"), Ok(rgb(0xFF, 0x52, 0x00)));
        assert_eq!(parse_css_color(b"#f508"), Ok(rgb(0xFF, 0x55, 0x00)));
    }

    #[test]
    fn rejects_malformed_hex() {
        for input in ["#", "#12", "#12345", "#GGGGGG", "#1234567"] {
            assert_eq!(
                parse_css_color(input.as_bytes()),
                Err(()),
                "accepted {input}"
            );
        }
    }

    #[test]
    fn parses_rgb_in_every_spelling() {
        assert_eq!(parse_css_color(b"rgb(255, 82, 0)"), Ok(rgb(0xFF, 0x52, 0x00)));
        assert_eq!(parse_css_color(b"rgb(255 82 0)"), Ok(rgb(0xFF, 0x52, 0x00)));
        assert_eq!(parse_css_color(b"RGB(255,82,0)"), Ok(rgb(0xFF, 0x52, 0x00)));
        assert_eq!(parse_css_color(b"rgba(255, 82, 0, 0.5)"), Ok(rgb(0xFF, 0x52, 0x00)));
        assert_eq!(parse_css_color(b"rgb(100%, 32.2%, 0%)"), Ok(rgb(0xFF, 0x52, 0x00)));
    }

    #[test]
    fn parses_hsl() {
        // hsl(18, 100%, 50%) is rgb(255, 77, 0) - a slightly deeper orange than
        // the #FF5200 the tool reports for the file this test came from.
        assert_eq!(parse_css_color(b"hsl(18, 100%, 50%)"), Ok(rgb(255, 77, 0)));
        assert_eq!(parse_css_color(b"hsl(18deg 100% 50%)"), Ok(rgb(255, 77, 0)));
        assert_eq!(parse_css_color(b"hsl(0, 0%, 100%)"), Ok(rgb(255, 255, 255)));
        assert_eq!(parse_css_color(b"hsl(0, 0%, 0%)"), Ok(rgb(0, 0, 0)));
        // Hue wraps rather than going out of range.
        assert_eq!(parse_css_color(b"hsl(378, 100%, 50%)"), Ok(rgb(255, 77, 0)));
    }

    #[test]
    fn parses_wide_gamut_colour_functions() {
        // The form that prompted this: a design tool's inline style.
        let p3 = parse_css_color(b"color(display-p3 1.0000 0.3216 0.0000)").unwrap();
        assert!(p3.r > 240, "expected a red-dominant result, got {p3:?}");
        // P3 primaries are wider than sRGB, so converting has to bring this back
        // down. Treating the encoded value as if it were linear skipped that and
        // landed on yellow.
        assert!(p3.g < 100, "expected a darker green than sRGB #FF5200, got {p3:?}");

        assert_eq!(parse_css_color(b"color(srgb 1 0 0)"), Ok(rgb(255, 0, 0)));
        assert_eq!(parse_css_color(b"color(srgb 0 0 0)"), Ok(rgb(0, 0, 0)));
    }

    #[test]
    fn a_wide_gamut_literal_is_close_enough_to_its_srgb_twin_to_match_it() {
        // The rule set comes from the rasterised preview, which reads the hex. The
        // markup also carries a display-p3 literal that resolves to a slightly
        // different value, and it wins in a browser. If it were further away than
        // the default tolerance, the saved file would keep the original colour in
        // the style attribute while the preview showed the replacement.
        let p3 = parse_css_color(b"color(display-p3 1.0000 0.3216 0.0000)").unwrap();
        let hex = Rgb::new(0xFF, 0x52, 0x00);

        let distance = p3.oklab().distance(hex.oklab());
        assert!(
            distance <= tolerance_to_distance(30),
            "display-p3 literal sits {distance} away, beyond the default tolerance"
        );
    }

    #[test]
    fn wide_gamut_conversion_stays_in_range() {
        for v in ["0", "0.25", "0.5", "0.75", "1"] {
            let arg = format!("color(display-p3 {v} {v} {v})");
            // Parsing must not panic or produce a nonsense value; the clamp inside
            // `linear_to_byte` is what keeps out-of-gamut primaries from wrapping.
            let _ = parse_css_color(arg.as_bytes()).expect("should parse");
        }
    }

    #[test]
    fn parses_named_colours() {
        assert_eq!(parse_css_color(b"red"), Ok(rgb(255, 0, 0)));
        assert_eq!(parse_css_color(b"RED"), Ok(rgb(255, 0, 0)));
        assert_eq!(parse_css_color(b"rebeccapurple"), Ok(rgb(102, 51, 153)));
        assert_eq!(parse_css_color(b"transparent"), Err(()), "not a colour to replace");
        assert_eq!(parse_css_color(b"currentColor"), Err(()), "indirect, not a literal");
    }

    #[test]
    fn every_named_colour_is_in_the_table() {
        // A typo would otherwise only show up as a colour that quietly refuses
        // to recolour.
        for name in [
            "aliceblue", "aqua", "beige", "black", "blue", "chartreuse", "coral",
            "crimson", "cyan", "fuchsia", "gold", "gray", "green", "indigo", "ivory",
            "khaki", "lime", "magenta", "maroon", "navy", "olive", "orange", "orchid",
            "pink", "plum", "purple", "red", "salmon", "silver", "tan", "teal",
            "tomato", "violet", "wheat", "white", "yellow",
        ] {
            assert!(named_color(name).is_some(), "{name} missing from the table");
        }
        assert!(named_color("notacolour").is_none());
    }

    // ---- finding literals in a document -----------------------------------

    #[test]
    fn finds_paint_attributes() {
        let svg = r##"<path fill="#FF5200" stroke="rgb(1,2,3)" d="M0 0"/>"##;
        let hexes = hexes_in(svg);
        assert!(hexes.contains(&"#FF5200".to_string()), "got {hexes:?}");
        assert!(hexes.contains(&"#010203".to_string()), "got {hexes:?}");
    }

    #[test]
    fn does_not_treat_a_fragment_identifier_as_a_colour() {
        // The reason this is a parser and not a substring search.
        let svg = r##"<linearGradient id="abc"><stop stop-color="#FF5200"/></linearGradient>"##;
        let found = collect_colors(svg.as_bytes());
        assert_eq!(found.len(), 1, "only the stop-color should match");
        assert_eq!(found[0].color, rgb(0xFF, 0x52, 0x00));
    }

    #[test]
    fn does_not_treat_a_url_reference_as_a_colour() {
        let svg = r##"<rect fill="url(#abc123)" stroke="#FF5200"/>"##;
        let found = collect_colors(svg.as_bytes());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].color, rgb(0xFF, 0x52, 0x00));
    }

    #[test]
    fn finds_colours_in_an_inline_style() {
        let svg = r##"<path style="fill:#FF5200;stroke:rgb(1,2,3);fill-opacity:1"/>"##;
        let hexes = hexes_in(svg);
        assert!(hexes.contains(&"#FF5200".to_string()), "got {hexes:?}");
        assert!(hexes.contains(&"#010203".to_string()), "got {hexes:?}");
    }

    #[test]
    fn finds_colours_in_a_style_element() {
        let svg = r##"<style>.a{fill:#FF5200}.b{stroke:#010203}</style>"##;
        assert_eq!(collect_colors(svg.as_bytes()).len(), 2);
    }

    #[test]
    fn finds_gradient_stop_colours() {
        let svg = r##"<linearGradient><stop stop-color="red"/><stop stop-color="blue"/></linearGradient>"##;
        let hexes = hexes_in(svg);
        assert!(hexes.contains(&"#FF0000".to_string()), "got {hexes:?}");
        assert!(hexes.contains(&"#0000FF".to_string()), "got {hexes:?}");
    }

    #[test]
    fn ignores_colours_in_non_paint_attributes() {
        // `d` is path data and `id` is an identifier; neither is a colour.
        let svg = r##"<path id="a" data-fill="#FF5200" d="M0 0 L1 1"/>"##;
        assert!(collect_colors(svg.as_bytes()).is_empty());
    }

    #[test]
    fn ignores_opacity_properties() {
        let svg = r##"<path fill="#FF5200" fill-opacity="0.5" stroke-opacity="1"/>"##;
        assert_eq!(collect_colors(svg.as_bytes()).len(), 1);
    }

    // ---- rewriting --------------------------------------------------------

    #[test]
    fn rewrites_a_matching_literal_in_place() {
        let (out, n) = recolor_to(r##"<path fill="#FF5200" d="M0 0"/>"##, "#FF5200", "#2876D2");
        assert_eq!(n, 1);
        assert_eq!(out, r##"<path fill="#2876D2" d="M0 0"/>"##);
    }

    #[test]
    fn leaves_the_rest_of_the_document_byte_identical() {
        let svg = r##"<?xml version="1.0"?>
<!-- a comment with #FF5200 in it -->
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 353 353">
  <path d="M173.328 0.0576346C174.571 0.0111346" fill="#FF5200" style="fill:#FF5200"/>
</svg>"##;
        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 2);

        assert!(out.contains("M173.328 0.0576346C174.571 0.0111346"));
        assert!(out.contains("<!-- a comment with #FF5200 in it -->"));
        assert!(out.contains(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 353 353">"##));
        assert!(out.contains(r##"fill="#2876D2""##));
    }

    #[test]
    fn a_colour_in_a_comment_is_not_replaced() {
        let svg = "<svg><!-- fill=\"#FF5200\" --><path fill=\"#FF5200\"/></svg>";
        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 1);
        assert!(out.contains("<!-- fill=\"#FF5200\" -->"), "the comment changed");
    }

    #[test]
    fn a_document_with_no_match_is_returned_unchanged() {
        let svg = r##"<svg><path fill="#2876D2"/></svg>"##;
        let (out, n) = recolor_to(svg, "#FF5200", "#000000");
        assert_eq!(n, 0);
        assert_eq!(out, svg);
    }

    #[test]
    fn an_empty_rule_list_changes_nothing() {
        let svg = r##"<svg><path fill="#FF5200"/></svg>"##;
        let (out, n) = recolor_svg_text(svg, &RecolorParams::default()).unwrap();
        assert_eq!(n, 0);
        assert_eq!(out, svg);
    }

    #[test]
    fn tolerance_still_decides_what_counts_as_a_different_colour() {
        // Same-paint floor in action: #FF5300 is one unit of green from
        // #FF5200, far closer than the notation gap, so it counts as the same
        // paint and is rewritten even at tolerance 0. This is the one place SVG
        // deliberately does not honour an exact match.
        let near = r##"<path fill="#FF5300"/>"##;
        let (out, n) = recolor_svg_text(near, &params_from(&[("#FF5200", "#000000")], 0)).unwrap();
        assert_eq!(n, 1, "a one-unit difference is the same colour");
        assert_eq!(out, r##"<path fill="#000000"/>"##);

        // The floor is not a licence to recolour things. At tolerance 0 the floor
        // is the only thing in play, and yellow is nowhere near it.
        let yellow = r##"<path fill="#FDE047"/>"##;
        let (out, n) = recolor_svg_text(yellow, &params_from(&[("#FF5200", "#000000")], 0)).unwrap();
        assert_eq!(n, 0, "the floor must not reach a visibly different colour");
        assert_eq!(out, yellow);
    }

    #[test]
    fn the_slider_still_governs_between_the_floor_and_the_top() {
        // The floor only ever grows the radius from below. Above it the slider is
        // what decides, so a colour the floor rejects but the slider accepts has
        // to be replaced - and tightening the slider has to tighten matching.
        let target = Rgb::new(0xFF, 0x52, 0x00);
        let ceiling = tolerance_to_distance(30);

        // Find a colour in the band between the floor and tolerance 30, rather than
        // hard-coding one. A hand-picked hex silently rots when either constant
        // changes, and the test would then pass while proving nothing.
        let probe = (0u16..=255)
            .map(|g| Rgb::new(0xFF, g as u8, 0x00))
            .find(|c| {
                let d = c.oklab().distance(target.oklab());
                d > SAME_PAINT_FLOOR && d <= ceiling
            })
            .expect("no colour falls between the floor and tolerance 30");

        let svg = format!(r##"<path fill="{}"/>"##, probe.to_hex());
        let loose = params_from(&[("#FF5200", "#000000")], 30);
        let strict = params_from(&[("#FF5200", "#000000")], 0);

        let (out, loose_n) = recolor_svg_text(&svg, &loose).unwrap();
        let (_, strict_n) = recolor_svg_text(&svg, &strict).unwrap();

        assert_eq!(loose_n, 1, "the slider should reach {}", probe.to_hex());
        assert_eq!(out, r##"<path fill="#000000"/>"##);
        assert_eq!(strict_n, 0, "below the slider it should not");
    }

    #[test]
    fn all_to_one_replaces_every_colour() {
        let svg = r##"<svg><path fill="#FF5200"/><path fill="#2876D2"/><path fill="red"/></svg>"##;
        let params = RecolorParams {
            mode: RecolorMode::AllToOne,
            all_to_one: Rgb::new(0xCB, 0xE7, 0x1F),
            ..Default::default()
        };
        let (out, n) = recolor_svg_text(svg, &params).unwrap();
        assert_eq!(n, 3);
        assert_eq!(out.matches("#CBE71F").count(), 3, "got {out}");
    }

    #[test]
    fn overlapping_spans_do_not_corrupt_the_output() {
        // A `style` attribute and a paint attribute on the same element, both
        // matched. Splicing has to run back to front or the offsets shift.
        let svg = r##"<path fill="#FF5200" style="stroke:#FF5200;fill:#FF5200"/>"##;
        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 3);
        assert!(!out.contains("#FF5200"), "a literal survived: {out}");
        assert_eq!(out.matches("#2876D2").count(), 3);
    }

    #[test]
    fn rewrites_every_literal_in_the_real_favicon_shape() {
        // The shape of the file that prompted all of this: a presentation
        // attribute and a CSS Color 4 value in the inline style, where the style
        // wins in a browser. Both must change or the edit does nothing visible.
        let svg = r##"<path d="M173.328 0.0576346" fill="#FF5200" style="fill:#FF5200;fill:color(display-p3 1.0000 0.3216 0.0000);fill-opacity:1;"/>"##;
        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");

        assert_eq!(n, 3, "got {n} replacements in {out}");
        assert!(!out.contains("#FF5200"), "a hex survived: {out}");
        assert_eq!(out.matches("#2876D2").count(), 3, "got {out}");
        assert!(out.contains("fill-opacity:1;"), "unrelated declarations changed");
        assert!(out.contains(r#"d="M173.328 0.0576346""#), "path data changed");
    }

    #[test]
    fn output_is_still_structurally_sound() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><g><path fill="#FF5200" style="stroke:#FF5200"/></g></svg>"##;
        let (out, _) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(out.matches('<').count(), out.matches('>').count());
        assert!(out.starts_with("<svg"));
        assert!(out.ends_with("</svg>"));
    }

    #[test]
    fn every_replaced_literal_becomes_a_valid_hex() {
        let svg = r##"<svg><path fill="red"/><path fill="hsl(18,100%,50%)"/><path style="stroke:rgb(255,82,0)"/></svg>"##;
        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 3);
        for residue in ["red\"", "hsl(", "rgb("] {
            assert!(!out.contains(residue), "{residue} survived in {out}");
        }
        assert_eq!(out.matches("#2876D2").count(), 3, "got {out}");
    }

    #[test]
    fn declared_colors_deduplicates() {
        let svg = r##"<path fill="#FF5200" stroke="#FF5200"/><path fill="#2876D2"/>"##;
        let declared = declared_colors(svg);
        assert_eq!(declared.len(), 2, "got {declared:?}");
        assert!(declared.contains(&rgb(0xFF, 0x52, 0x00)));
        assert!(declared.contains(&rgb(0x28, 0x76, 0xD2)));
    }

    #[test]
    fn distinct_paints_folds_two_notations_of_one_colour() {
        // The file that prompted this declares its paint as a hex *and* as a
        // display-p3 literal that resolves a hair away. It is one colour, and the
        // user is looking at one orange.
        let svg = r##"<path fill="#FF5200" style="fill:#FF5200;fill:color(display-p3 1.0000 0.3216 0.0000)"/>"##;
        let paints = distinct_paints(svg);

        assert_eq!(paints.len(), 1, "got {paints:?}");
        assert_eq!(paints[0].color, rgb(0xFF, 0x52, 0x00));
        assert_eq!(paints[0].count, 3, "all three literals belong to that paint");
    }

    #[test]
    fn distinct_paints_keeps_genuinely_separate_colours() {
        // The other real file: three well-separated paints across five literals.
        // Folding must not merge them into one.
        let svg = r##"
            <path style="fill:rgb(32,31,31)"/>
            <path style="fill:rgb(255,254,238)"/>
            <path style="fill:rgb(255,254,238)"/>
            <path style="fill:rgb(244,123,47)"/>
            <path style="fill:rgb(255,254,238)"/>
        "##;
        let paints = distinct_paints(svg);

        let hexes: Vec<_> = paints.iter().map(|p| p.color.to_hex()).collect();
        assert_eq!(paints.len(), 3, "got {hexes:?}");
        assert!(hexes.contains(&"#201F1F".to_string()), "got {hexes:?}");
        assert!(hexes.contains(&"#FFFEEE".to_string()), "got {hexes:?}");
        assert!(hexes.contains(&"#F47B2F".to_string()), "got {hexes:?}");

        // Counts are literals, and coverage is their share.
        let cream = paints.iter().find(|p| p.color.to_hex() == "#FFFEEE").unwrap();
        assert_eq!(cream.count, 3);
        assert!((cream.coverage - 0.6).abs() < 1e-6, "got {}", cream.coverage);
    }

    #[test]
    fn distinct_paints_of_a_document_with_no_colours_is_empty() {
        assert!(distinct_paints(r##"<svg><path d="M0 0"/></svg>"##).is_empty());
        assert!(distinct_paints("").is_empty());
    }

    #[test]
    fn distinct_paints_order_is_stable() {
        // The list must not reshuffle between opens. Equal counts break the tie on
        // the hex string, because `Rgb` is not `Ord`.
        let svg = r##"<path fill="#2876D2"/><path fill="#FF5200"/><path fill="#201F1F"/>"##;
        let first: Vec<_> = distinct_paints(svg).iter().map(|p| p.color.to_hex()).collect();
        let second: Vec<_> = distinct_paints(svg).iter().map(|p| p.color.to_hex()).collect();
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
    }

    #[test]
    fn distinct_paints_matches_what_the_same_paint_floor_would_rewrite() {
        // The list and the save have to agree about what "the same paint" means.
        // If they drifted, a colour could be listed but never replaced, or
        // replaced without ever being listed.
        let svg = r##"<path fill="#FF5200" style="fill:color(display-p3 1.0000 0.3216 0.0000)"/>"##;
        let listed = distinct_paints(svg);
        assert_eq!(listed.len(), 1);

        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 2, "both literals must still be rewritten");
        assert_eq!(out.matches("#2876D2").count(), 2);
    }

    // ---- CSS structure --------------------------------------------------
    //
    // Each of these found its colour in an empty list before: the scanner read
    // the construct as one long property value and ran to the end of the block.

    #[test]
    fn media_query_block_is_descended_into() {
        // `min-width:100px)` reads exactly like a declaration, and swallowing it
        // takes the only colour in the file with it.
        let svg = r##"<style>@media (min-width:100px){.a{fill:#FF5200}}</style>"##;
        assert_eq!(distinct_paints(svg).len(), 1, "got {:?}", distinct_paints(svg));

        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 1);
        assert!(out.contains("#2876D2"), "got {out}");
    }

    #[test]
    fn a_selector_is_not_a_declaration() {
        // `:root{` has to be recognised as a rule, or its block is treated as
        // the value of a property with an empty name.
        let svg = r##"<style>:root{--brand:#FF5200}.a{fill:#2876D2}</style>"##;
        let paints = distinct_paints(svg);
        let hexes: Vec<_> = paints.iter().map(|p| p.color.to_hex()).collect();
        assert!(hexes.contains(&"#FF5200".to_string()), "got {hexes:?}");
        assert!(hexes.contains(&"#2876D2".to_string()), "got {hexes:?}");
    }

    #[test]
    fn descendant_and_pseudo_selectors_are_not_declarations() {
        for selector in [".a", "g .a", "a:hover", "#id", "*", "svg > g"] {
            let svg = format!(r#"<style>{selector}{{fill:#FF5200}}</style>"#);
            let paints = distinct_paints(&svg);
            assert_eq!(
                paints.len(),
                1,
                "selector {selector:?} lost its colour, got {:?}",
                paints.iter().map(|p| p.color.to_hex()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn important_is_not_part_of_the_value() {
        let attribute = r##"<rect style="fill:#FF5200 !important"/>"##;
        let block = r##"<style>.a{fill:#FF5200 !important}</style>"##;

        for svg in [attribute, block] {
            assert_eq!(distinct_paints(svg).len(), 1, "got {:?}", distinct_paints(svg));

            let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
            assert_eq!(n, 1, "{svg}");
            // The modifier must survive; only the colour changes.
            assert!(out.contains("!important"), "got {out}");
            assert!(!out.contains("#FF5200"), "got {out}");
        }
    }

    #[test]
    fn important_without_a_space_is_still_a_modifier() {
        let (out, n) = recolor_to(
            r##"<rect style="fill:#FF5200!important"/>"##,
            "#FF5200",
            "#2876D2",
        );
        assert_eq!(n, 1);
        assert!(out.contains("!important"), "got {out}");
    }

    #[test]
    fn a_custom_property_holding_a_colour_is_a_paint() {
        // `--brand` is not itself a paint, but it is the only place the colour
        // exists, so recolouring must reach it.
        let svg = r##"<style>:root{--brand:#FF5200}.a{fill:var(--brand)}</style>"##;
        let paints = distinct_paints(svg);
        assert_eq!(paints.len(), 1, "got {:?}", distinct_paints(svg));

        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 1, "got {out}");
        // Rewritten at the definition, so every reference follows.
        assert!(out.contains("--brand:#2876D2"), "got {out}");
    }

    #[test]
    fn a_custom_property_holding_a_number_is_not_a_colour() {
        let svg = r##"<style>:root{--gap:10px}.a{fill:var(--gap, #FF5200)}</style>"##;
        let hexes: Vec<_> = distinct_paints(svg)
            .iter()
            .map(|p| p.color.to_hex())
            .collect();
        assert_eq!(hexes, vec!["#FF5200".to_string()], "got {hexes:?}");
    }

    #[test]
    fn var_fallback_is_used_when_undefined() {
        let svg = r##"<rect style="fill:var(--missing, #FF5200)"/>"##;
        assert_eq!(distinct_paints(svg).len(), 1, "got {:?}", distinct_paints(svg));

        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 1);
        assert!(out.contains("var(--missing, #2876D2)"), "got {out}");
    }

    #[test]
    fn var_chains_resolve() {
        let svg =
            r##"<style>:root{--a:#FF5200;--b:var(--a)}.x{fill:var(--b)}</style>"##;
        assert_eq!(distinct_paints(svg).len(), 1, "got {:?}", distinct_paints(svg));

        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 1, "got {out}");
        assert!(out.contains("--a:#2876D2"), "got {out}");
    }

    #[test]
    fn a_var_cycle_terminates() {
        // Must not recurse forever, and must not invent a colour.
        let svg = r##"<style>:root{--a:var(--b);--b:var(--a)}.x{fill:var(--a)}</style>"##;
        let _ = distinct_paints(svg);
    }

    #[test]
    fn a_brace_inside_a_value_does_not_end_it_early() {
        let svg = r##"<rect style="fill:#FF5200;--x:{a:b}"/>"##;
        let hexes: Vec<_> = distinct_paints(svg)
            .iter()
            .map(|p| p.color.to_hex())
            .collect();
        assert_eq!(hexes, vec!["#FF5200".to_string()], "got {hexes:?}");
    }

    #[test]
    fn an_unterminated_block_does_not_hang() {
        // No closing brace anywhere in the input.
        let _ = distinct_paints(r##"<style>.a{fill:#FF5200"##);
        let _ = distinct_paints(r##"<style>(min-width:100px"##);
        let _ = distinct_paints(r##"<rect style="fill:#FF5200"##);
    }

    #[test]
    fn an_unterminated_quote_does_not_hang() {
        let _ = distinct_paints(r##"<rect style="fill:#FF5200;--x:'unclosed"/>"##);
    }

    #[test]
    fn a_css_comment_in_a_value_is_skipped() {
        let svg = r##"<style>.a{fill:/* brand */#FF5200}</style>"##;
        assert_eq!(distinct_paints(svg).len(), 1, "got {:?}", distinct_paints(svg));

        let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
        assert_eq!(n, 1);
        // The comment is left in place; only the colour after it changes.
        assert!(out.contains("/* brand */#2876D2"), "got {out}");
    }

    #[test]
    fn a_comment_holding_a_separator_does_not_end_the_value() {
        // The `;` inside the comment must not terminate the declaration, or the
        // colour after it is never reached.
        let svg = r##"<style>.a{fill:/* ; */#FF5200}</style>"##;
        assert_eq!(distinct_paints(svg).len(), 1, "got {:?}", distinct_paints(svg));
    }

    #[test]
    fn a_comment_holding_a_brace_does_not_unbalance_the_block() {
        let svg = r##"<style>/* } */.a{fill:#FF5200}</style>"##;
        assert_eq!(distinct_paints(svg).len(), 1, "got {:?}", distinct_paints(svg));
    }

    #[test]
    fn a_rewrite_lands_on_the_value_not_on_the_text_around_it() {
        // The offsets here are the whole point: an off-by-a-few here silently
        // rewrites the wrong bytes and corrupts the document.
        for (svg, expected) in [
            // The fallback sits after `, ` and before the closing paren.
            (
                r##"<rect style="fill:var(--missing, #FF5200)"/>"##,
                r##"fill:var(--missing, #2876D2)"##,
            ),
            // No space after the comma.
            (
                r##"<rect style="fill:var(--missing,#FF5200)"/>"##,
                r##"fill:var(--missing,#2876D2)"##,
            ),
            // Leading comment plus surrounding whitespace.
            (
                r##"<rect style="fill:  /* c */  #FF5200  "/>"##,
                r##"/* c */  #2876D2"##,
            ),
        ] {
            let (out, n) = recolor_to(svg, "#FF5200", "#2876D2");
            assert_eq!(n, 1, "{svg}");
            assert!(out.contains(expected), "in {out}\nfor {svg}");
        }
    }

    #[test]
    fn a_value_padded_with_whitespace_is_still_a_value() {
        // `fill: #FF5200` with a space either side is how every formatter writes
        // it, and it used to be read as a shorthand and skipped.
        for value in [
            " #FF5200 ",
            "\t#FF5200\t",
            "  #FF5200;",
            "\n  #FF5200  \n",
        ] {
            let svg = format!(r#"<rect style="fill:{value}"/>"#);
            assert_eq!(
                distinct_paints(&svg).len(),
                1,
                "value {value:?} lost its colour: {:?}",
                distinct_paints(&svg)
            );

            let (out, n) = recolor_to(&svg, "#FF5200", "#2876D2");
            assert_eq!(n, 1, "value {value:?}");
            assert!(out.contains("#2876D2"), "value {value:?} -> {out}");
        }
    }

    #[test]
    fn a_whitespace_padded_value_keeps_its_spacing() {
        let (out, _) = recolor_to(r#"<rect style="fill:   #FF5200   "/>"#, "#FF5200", "#2876D2");
        assert!(
            out.contains(r#"fill:   #2876D2   "#),
            "spacing must survive, got {out}"
        );
    }

    #[test]
    fn a_rewrite_does_not_corrupt_the_document() {
        // Belt and braces: whatever happens, the output must still be the same
        // document with one colour swapped, never a mangled one.
        for svg in [
            r##"<rect style="fill:var(--missing, #FF5200)"/>"##,
            r##"<style>@media (min-width:1px){.a{fill:#FF5200}}</style>"##,
            r##"<style>:root{--a:#FF5200}.b{fill:var(--a)}</style>"##,
            r##"<style>.a{fill:/* ; */#FF5200}</style>"##,
        ] {
            let (out, _) = recolor_to(svg, "#FF5200", "#2876D2");
            let before = svg.matches(['<', '>']).count();
            let after = out.matches(['<', '>']).count();
            assert_eq!(before, after, "markup was damaged: {svg} -> {out}");
            assert!(!out.contains("#FF5200"), "{svg} -> {out}");
        }
    }

    // ---- file round trip --------------------------------------------------

    #[test]
    fn writes_a_real_svg_file() {
        let dir = std::env::temp_dir().join("wheel_svg_recolor_write");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("in.svg");
        std::fs::write(&input, r##"<svg><path fill="#FF5200"/></svg>"##).unwrap();

        let output = dir.join("nested").join("out.svg");
        let result =
            recolor_svg_file(&input, &output, &params_from(&[("#FF5200", "#2876D2")], 30)).unwrap();

        assert_eq!(result.replaced, 1);
        assert_eq!(result.found, 1);
        assert!(!result.unchanged);
        assert_eq!(
            std::fs::read_to_string(&output).unwrap(),
            r##"<svg><path fill="#2876D2"/></svg>"##
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_unchanged_when_nothing_matched() {
        let dir = std::env::temp_dir().join("wheel_svg_recolor_unchanged");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("in.svg");
        let source = r##"<svg><path fill="#2876D2"/></svg>"##;
        std::fs::write(&input, source).unwrap();

        let result = recolor_svg_file(
            &input,
            &dir.join("out.svg"),
            &params_from(&[("#FF5200", "#000000")], 30),
        )
        .unwrap();

        assert!(result.unchanged);
        assert_eq!(result.replaced, 0);
        assert_eq!(std::fs::read_to_string(dir.join("out.svg")).unwrap(), source);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_a_missing_file_rather_than_writing_an_empty_one() {
        let missing = Path::new("C:/definitely/not/here.svg");
        assert!(
            recolor_svg_file(missing, Path::new("C:/out.svg"), &RecolorParams::default()).is_err()
        );
    }

    #[test]
    fn handles_a_file_with_no_colours_at_all() {
        let dir = std::env::temp_dir().join("wheel_svg_recolor_nocolour");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("in.svg");
        let source = r##"<svg viewBox="0 0 10 10"><path d="M0 0 L1 1"/></svg>"##;
        std::fs::write(&input, source).unwrap();

        let result =
            recolor_svg_file(&input, &dir.join("out.svg"), &RecolorParams::default()).unwrap();

        assert_eq!(result.found, 0);
        assert_eq!(result.replaced, 0);
        assert_eq!(std::fs::read_to_string(dir.join("out.svg")).unwrap(), source);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Helper: every colour a document declares, as hex strings.
    fn hexes_in(svg: &str) -> Vec<String> {
        collect_colors(svg.as_bytes())
            .iter()
            .map(|s| s.color.to_hex())
            .collect()
    }
}
