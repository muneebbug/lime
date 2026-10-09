//! Colour replacement for still images.
//!
//! Two things set this apart from a naive "compare RGB tuples" implementation:
//!
//! * Matching happens in **OKLab**, a perceptually uniform space. Two colours
//!   that a person calls "the same blue" sit ~0.02 apart in OKLab even when
//!   their RGB values are far apart numerically, while colours that look
//!   clearly different sit much further out. Matching in RGB misses gradients
//!   and shaded regions; matching in OKLab does not.
//! * Fully transparent pixels are **skipped**. A decoded PNG keeps whatever RGB
//!   bytes were under the transparent area, so recolouring them would paint
//!   colour onto pixels that are not visible and shows up as a halo when the
//!   result is later composited.
//!
//! Alpha itself is never touched.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use tracing::info;

/// An 8-bit RGB colour, no alpha.
///
/// Alpha is carried separately throughout: it is part of the pixel, not of the
/// colour the user picks or types.
///
/// Serialises as a `#RRGGBB` string; see `hex_string` below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Rebuild a colour from the packed form used for counting.
    ///
    /// The layout is `0x00RRGGBB` in big-endian byte order, so the unpack is
    /// three shifts down from the top rather than a byte-indexed slice.
    fn from_packed(packed: u32) -> Self {
        Self::new(
            (packed >> 16) as u8,
            (packed >> 8) as u8,
            packed as u8,
        )
    }

    /// Parse `#rgb`, `#rrggbb`, with or without the leading `#`.
    ///
    /// Case insensitive, and tolerant of surrounding whitespace, because these
    /// strings are typed by hand in the UI as often as they are pasted.
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex: String = text
            .trim()
            .trim_start_matches('#')
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        let nibble = |c: char| c.to_digit(16).map(|d| d as u8);

        let digits: Vec<char> = hex.chars().collect();
        match digits.as_slice() {
            // #abc - each digit doubled, so #abc is #aabbcc.
            [r, g, b] => Some(Self::new(
                nibble(*r)? * 17,
                nibble(*g)? * 17,
                nibble(*b)? * 17,
            )),
            [r, r2, g, g2, b, b2] => Some(Self::new(
                nibble(*r)? * 16 + nibble(*r2)?,
                nibble(*g)? * 16 + nibble(*g2)?,
                nibble(*b)? * 16 + nibble(*b2)?,
            )),
            _ => None,
        }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// Colours cross the Tauri bridge as `#RRGGBB` strings rather than `{r, g, b}`
/// objects.
///
/// The frontend's colour inputs, swatches and hex fields all work in hex
/// strings, so this keeps the value identical on both sides of the IPC boundary
/// and means a rule read back from disk can be pasted straight into the picker.
mod hex_string {
    use super::Rgb;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(color: &Rgb, serializer: S) -> Result<S::Ok, S::Error> {
        color.to_hex().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Rgb, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rgb::from_hex(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("not a colour: {text:?}")))
    }
}

/// How far apart two colours may be and still count as the same paint.
///
/// The worst-case error of a premultiply/unpremultiply round trip is half a
/// quantisation step in the stored byte, amplified by `255 / a` on the way back
/// out, plus half a step again when the result is rounded. Rounded up that is
/// `255 / a + 1`: 2 units at full alpha, which comfortably covers the ordinary
/// 1-unit disagreement rounding already produces, and 9 units at alpha 32.
fn slack_for(alpha: u8) -> i32 {
    (255 / u32::from(alpha).max(1)) as i32 + 1
}

/// Chebyshev distance between two packed colours.
///
/// Chebyshev rather than Euclidean because the un-premultiply error is bounded
/// per channel independently - each byte is scaled by `255 / a` on its own - so
/// "close enough" is genuinely a per-channel box, and a box is what can be
/// searched efficiently.
fn channel_distance(a: u32, b: u32) -> i32 {
    let dr = ((a >> 16) as i32) - ((b >> 16) as i32);
    let dg = ((a >> 8) as i32 & 0xFF) - ((b >> 8) as i32 & 0xFF);
    let db = (a as i32 & 0xFF) - (b as i32 & 0xFF);
    dr.abs().max(dg.abs()).max(db.abs())
}

/// Nearest entry of the sorted `opaque` list within `slack`, if there is one.
fn nearest_opaque(opaque: &[u32], key: u32, slack: i32) -> Option<u32> {
    // The packed value orders lexicographically by (r, g, b), so the window is
    // found by advancing to the first entry that could be inside the box and
    // stopping at the first that cannot.
    let start = opaque.partition_point(|k| {
        channel_distance(*k, key) > slack && *k < key
    });

    opaque[start..]
        .iter()
        .take_while(|k| channel_distance(**k, key) <= slack)
        .min_by_key(|k| channel_distance(**k, key))
        .copied()
}

/// Reduce one channel to the precision that its own alpha can actually carry.
///
/// A rasterised SVG arrives premultiplied and is un-premultiplied back to
/// straight alpha, and that round trip is lossy: at alpha 40, `#FF5200` comes
/// back as `#FF5300`, at alpha 24 as `#FF5500`. Every one of those is the *same*
/// paint, differing only by rounding. Keying on the raw bytes reported a
/// one-colour logo as six colours.
///
/// Rather than quantising to a fixed number of bits - which would merge genuinely
/// distinct colours in a gradient and re-break the exactness this function
/// exists to provide - the step is derived from the uncertainty. Storing a channel
/// at 8 bits behind `a` bits of alpha leaves a worst-case error of `255 / (2a)`
/// units, so that is the grid, and no finer. At alpha 255 the step is 1 and the
/// value passes through untouched; at alpha 40 it is 3.
///
/// The result stays a colour the user can act on: rounding lands on the value
/// nearest the true paint, not an invented one.
fn pack_channel(value: u8, alpha: u8) -> u32 {
    if alpha >= 255 {
        return u32::from(value);
    }

    // Worst-case rounding error is half a quantisation step, hence the 2a.
    let step = (255 / (2 * u32::from(alpha))).max(1);
    // Round to the nearest multiple of the step, then clamp back into range: the
    // largest multiple below 255 can overshoot once rounding is applied.
    let rounded = (u32::from(value) + step / 2) / step * step;
    rounded.min(255) as u32
}

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        hex_string::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        hex_string::deserialize(deserializer)
    }
}

impl Rgb {
    pub fn oklab(self) -> Lab {
        let r = srgb_to_linear(self.r);
        let g = srgb_to_linear(self.g);
        let b = srgb_to_linear(self.b);

        let long = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
        let medium = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
        let short = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

        let long = long.cbrt();
        let medium = medium.cbrt();
        let short = short.cbrt();

        Lab {
            l: 0.2104542553 * long + 0.7936177850 * medium - 0.0040720468 * short,
            a: 1.9779984951 * long - 2.4285922050 * medium + 0.4505937099 * short,
            b: 0.0259040371 * long + 0.7827717662 * medium - 0.8086757660 * short,
        }
    }
}
fn srgb_to_linear(channel: u8) -> f32 {
    let c = channel as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> u8 {
    let c = value.clamp(0.0, 1.0);
    let encoded = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lab {
    l: f32,
    a: f32,
    b: f32,
}

impl Lab {
    pub fn distance(self, other: Lab) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        (dl * dl + da * da + db * db).sqrt()
    }
}

fn lab_to_rgb(lab: Lab) -> Rgb {
    let long = lab.l + 0.3963377774 * lab.a + 0.2158037573 * lab.b;
    let medium = lab.l - 0.1055613458 * lab.a - 0.0638541728 * lab.b;
    let short = lab.l - 0.0894841775 * lab.a - 1.2914855480 * lab.b;

    let long = long * long * long;
    let medium = medium * medium * medium;
    let short = short * short * short;

    let r = 4.0767416621 * long - 3.3077115913 * medium + 0.2309699292 * short;
    let g = -1.2684380046 * long + 2.6097574011 * medium - 0.3413193965 * short;
    let b = -0.0041960863 * long - 0.7034186147 * medium + 1.7076147010 * short;

    Rgb::new(linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b))
}

/// Convert the UI's 0-100 tolerance into an OKLab distance.
///
/// The scale is deliberately non-linear. Perceptual difference grows faster than
/// the slider position in the low range, where the interesting work is: at
/// tolerance 0 only an exact match is replaced, and by 30 the nearest shades of
/// one colour are caught without touching the next colour along. A plain linear
/// map spends most of its travel in the top half, where everything blurs
/// together.
pub fn tolerance_to_distance(tolerance: u8) -> f32 {
    let t = f32::from(tolerance.min(100)) / 100.0;
    t.powf(1.5) * 0.5
}

/// One "replace this colour with that one" instruction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RecolorRule {
    pub from: Rgb,
    pub to: Rgb,
}

/// What the tool should do with the pixels it matches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecolorMode {
    /// Apply the rule list, leaving everything unmatched untouched.
    #[default]
    Replace,
    /// Collapse every visible pixel onto a single colour.
    AllToOne,
}

/// Fully described recolour job.
///
/// Defaults are a no-op, so `RecolorParams::default()` returns the image
/// unchanged. That makes "nothing selected" a safe state to render rather than
/// a special case every caller has to guard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecolorParams {
    #[serde(default)]
    pub mode: RecolorMode,
    #[serde(default)]
    pub rules: Vec<RecolorRule>,
    /// Target colour for `AllToOne`.
    #[serde(default = "default_all_to_one")]
    pub all_to_one: Rgb,
    /// 0 = exact match only, 100 = merges visibly different colours.
    #[serde(default = "default_tolerance")]
    pub tolerance: u8,
    /// For `AllToOne`, keep each pixel's lightness so the image keeps its
    /// shading and detail instead of flattening into a silhouette.
    ///
    /// Off by default. "All to one colour" is read literally: paint everything
    /// that colour. Carrying the shading through is the surprising result, and
    /// has to be asked for. The UI has an explicit toggle for it.
    #[serde(default)]
    pub preserve_shading: bool,
}

fn default_all_to_one() -> Rgb {
    Rgb::new(203, 231, 31)
}

fn default_tolerance() -> u8 {
    30
}

impl Default for RecolorParams {
    fn default() -> Self {
        Self {
            mode: RecolorMode::Replace,
            rules: Vec::new(),
            all_to_one: default_all_to_one(),
            tolerance: default_tolerance(),
            preserve_shading: false,
        }
    }
}

impl RecolorParams {
    /// True when the params would leave the image alone.
    pub fn is_noop(&self) -> bool {
        match self.mode {
            RecolorMode::Replace => self.rules.is_empty(),
            RecolorMode::AllToOne => false,
        }
    }
}

/// A colour found in the image, with how much of the image it covers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExtractedColor {
    pub color: Rgb,
    /// Visible pixels of exactly this colour, at any alpha above the noise
    /// floor. Partial-alpha pixels count too: a colour at 40% opacity over a
    /// transparent background still looks like that colour.
    pub count: u64,
    /// `count` divided by the number of visible pixels, 0.0 to 1.0.
    pub coverage: f32,
}

/// Alpha at or below which a pixel is treated as empty rather than a faint
/// colour.
///
/// Antialiased edges are where almost all of a flat vector shape's boundary
/// lives. Keeping them is the point - recolouring a JPEG or a PNG logo has to
/// reach the pixels along its outline, or the result has a hard original-colour
/// fringe. But at the very bottom of the ramp the RGB under a near-invisible
/// pixel is quantisation noise, and counting it invents colours that are not
/// there.
///
/// This is far more aggressive than `trim`'s 8/255. That one has to find a
/// bounding box and a stray value of 5 near a transparent border would inflate
/// the crop. Here a pixel only contributes a colour to a list a human reads, so
/// the bar is set where the colour is genuinely visible.
const EXTRACTION_ALPHA_FLOOR: u8 = 32;

/// The result of scanning an image for its colours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorExtraction {
    /// Colours ordered by pixel count, highest first.
    pub colors: Vec<ExtractedColor>,
    /// How many distinct visible colours the image contains in total. This is
    /// often much larger than `colors.len()` - a photograph can have millions -
    /// which is why it is reported separately.
    pub total_unique: u64,
    /// Visible pixels considered. Pixels at or below `EXTRACTION_ALPHA_FLOOR`
    /// are excluded: below that the colour is not really there to be replaced.
    pub visible_pixels: u64,
    /// True when `total_unique` exceeds the number of colours returned.
    pub truncated: bool,
}

/// A colour in the extracted list, plus what one rule from it would repaint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorGroup {
    /// The colour a rule should use as its source.
    ///
    /// The most common colour not already covered by a nearer swatch, so the
    /// swatch the user clicks is one they can see a lot of rather than an
    /// arbitrary boundary case.
    pub color: Rgb,
    /// Distinct listed colours a single rule from `color` would repaint.
    ///
    /// This is a radius around the swatch, not a share of the list, so two
    /// nearby swatches can each claim the same colour. That overlap is the
    /// honest answer: those pixels really are in range of both rules, and which
    /// one wins is decided at recolour time by whichever is nearer. Reporting a
    /// partition instead would understate the blast radius, and a swatch that
    /// says 268 when the click repaints 316 is the bug this whole change exists
    /// to remove.
    pub merged: u64,
    /// Visible pixels a rule from `color` is expected to repaint.
    ///
    /// Derived from the listed colours, so it runs a few per cent under the
    /// truth where extraction folded a translucent blend onto a neighbouring
    /// entry while the rule still matched it by radius. Measured worst case on a
    /// real lossy image: 6%. Enough to size the change, not to bill by.
    pub count: u64,
    /// `count` as a share of the visible pixels, 0.0 to 1.0.
    ///
    /// Overlaps with its neighbours for the same reason `merged` does, so these
    /// do not sum to 1 across swatches.
    pub coverage: f32,
}

/// Fold an exact colour list into the swatches a rule would actually repaint.
///
/// The extracted list is a census, but a recolour rule is a sphere: at any
/// tolerance above zero, one rule already repaints every colour within
/// `tolerance_to_distance` of its source. So an exact list is not describing the
/// edit the user is about to make - on a lossy image it describes hundreds of
/// shades that were always going to move together. This returns one swatch per
/// distinct region of the image, at the current tolerance.
///
/// Two properties this must keep, because breaking either reintroduces the
/// problem:
///
/// * **The swatch count is the rule's radius.** Each swatch reports how many
///   colours one rule from it would catch, so clicking it does what the badge
///   said. A partition of the list is tidier but lies about the single-rule case,
///   which is the common one.
/// * **Deterministic order.** Representatives are chosen in order of pixel
///   count, with equal counts broken by hex, because `Rgb` is not `Ord`. Hash
///   order left to itself made the swatch count drift between runs of the same
///   file - 20, 21 then 22 - and a list that reshuffles on every open cannot be
///   trusted.
pub fn group_colors(colors: &[ExtractedColor], tolerance: u8) -> Vec<ColorGroup> {
    let radius = tolerance_to_distance(tolerance);

    let mut ordered: Vec<&ExtractedColor> = colors.iter().collect();
    ordered.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.color.to_hex().cmp(&b.color.to_hex()))
    });

    let prepared: Vec<(Lab, Rgb, u32)> = ordered
        .iter()
        .map(|entry| (entry.color.oklab(), entry.color, entry.count as u32))
        .collect();

    // One representative per region: a colour that no closer swatch already
    // reaches. Seeded in pixel order, so the biggest area of the image gets to
    // name its own swatch.
    let mut seeds: Vec<(Lab, Rgb)> = Vec::new();
    for (lab, color, _) in &prepared {
        if !seeds.iter().any(|(seed, _)| seed.distance(*lab) <= radius) {
            seeds.push((*lab, *color));
        }
    }

    let total_pixels: u64 = colors.iter().map(|c| c.count).sum();
    let denominator = total_pixels.max(1) as f32;

    seeds
        .into_iter()
        .map(|(seed, color)| {
            let mut merged = 0u64;
            let mut pixels = 0u64;
            for (lab, _, count) in &prepared {
                if seed.distance(*lab) <= radius {
                    merged += 1;
                    pixels += u64::from(*count);
                }
            }
            ColorGroup {
                color,
                merged,
                count: pixels,
                coverage: pixels as f32 / denominator,
            }
        })
        .collect()
}

/// Scan an image for every distinct colour it contains.
///
/// Exact on RGB, not quantised: two pixels that differ by a single unit in any
/// channel are reported as two colours. That is the whole point of the feature -
/// a gradient really does contain thousands of colours, and hiding that behind a
/// palette would make the tool lie about what is in the file.
///
/// Alpha decides only whether a pixel counts at all, never *what* colour it is.
/// Keying on RGB alone is what makes this correct for vector art: a flat-fill
/// SVG rasterises with antialiased edges, so its one declared fill arrives as the
/// same orange at a few dozen alpha levels. Keying on RGB-plus-alpha reported
/// each of those as a separate colour and told the user a single-colour logo had
/// six. See `EXTRACTION_ALPHA_FLOOR` for the other end of the trade.
///
/// Colours are counted by sorting the packed RGB values and run-length encoding
/// equal neighbours, which is faster and far more predictable in memory than a
/// hash map (a 12-megapixel photo can hold 16 million distinct colours; the hash
/// map would be the memory bottleneck, the sort is not).
///
/// Returns the most-used `max_colors` colours. `max_colors` exists only to keep
/// the response and the UI list bounded, never to merge colours together.
pub fn extract_colors(img: &DynamicImage, max_colors: usize) -> ColorExtraction {
    let rgba = img.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;

    /*
     * Two passes, and the reason is that a partial-alpha pixel cannot be trusted
     * to name its own colour.
     *
     * A rasterised SVG is composited premultiplied and then un-premultiplied back
     * to straight alpha by the decoder, and that round trip is lossy: `#FF5200`
     * at alpha 40 comes back as `#FF5300`, at alpha 24 as `#FF5500`. Those are all
     * the same paint, differing only by rounding, but they key to different
     * colours. Keying on raw bytes reported a single-colour logo as six.
     *
     * Fully opaque pixels have no such problem - their bytes are exactly what the
     * artist wrote - so they define the palette. A partial-alpha pixel is then
     * attributed to the opaque colour it was probably meant to be, and only
     * contributes a new colour if nothing close enough already exists.
     *
     * Attribution has to happen after the opaque colours are known, which is why
     * this cannot be a single streaming pass.
     */
    let mut visible_pixels: u64 = 0;
    let mut opaque: Vec<u32> = Vec::with_capacity(width * height);
    // (packed colour, alpha). Alpha is carried alongside because it decides both
    // the search radius and the fallback rounding.
    let mut translucent: Vec<(u32, u8)> = Vec::new();
    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        if a <= EXTRACTION_ALPHA_FLOOR {
            continue;
        }
        visible_pixels += 1;
        let key = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
        if a == 255 {
            opaque.push(key);
        } else {
            translucent.push((key, a));
        }
    }

    /*
     * Two views of the opaque pixels. `keys` keeps one entry per *pixel*, because
     * that is what the run-length pass counts; deduplicating it here would lose
     * every frequency. `palette` is the sorted unique set used for lookups.
     */
    let mut palette = opaque.clone();
    palette.sort_unstable();
    palette.dedup();

    let mut keys: Vec<u32> = opaque;

    for (key, alpha) in translucent {
        match nearest_opaque(&palette, key, slack_for(alpha)) {
            Some(matched) => keys.push(matched),
            None => keys.push(
                (pack_channel((key >> 16) as u8, alpha) << 16)
                    | (pack_channel((key >> 8) as u8, alpha) << 8)
                    | pack_channel(key as u8, alpha),
            ),
        }
    }

    // Counted before the dedup above, so this is pixels, not distinct colours.

    keys.sort_unstable();

    // Walk the sorted run-lengths, then order them by how large each run is.
    let mut runs: Vec<(u32, u64)> = Vec::new();
    let mut i = 0;
    while i < keys.len() {
        let key = keys[i];
        let mut run = 1;
        while i + run < keys.len() && keys[i + run] == key {
            run += 1;
        }
        runs.push((key, run as u64));
        i += run;
    }

    let total_unique = runs.len() as u64;
    runs.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let denominator = visible_pixels.max(1) as f32;
    let colors: Vec<ExtractedColor> = runs
        .into_iter()
        .take(max_colors)
        .map(|(key, count)| ExtractedColor {
            color: Rgb::from_packed(key),
            count,
            coverage: count as f32 / denominator,
        })
        .collect();

    ColorExtraction {
        truncated: total_unique > colors.len() as u64,
        colors,
        total_unique,
        visible_pixels,
    }
}

/// Apply `params` to an image, in memory.
///
/// Returns a new `DynamicImage`; the input is untouched.
pub fn recolor(img: &DynamicImage, params: &RecolorParams) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    apply_to_rgba(&mut rgba, params);
    DynamicImage::ImageRgba8(rgba)
}

/// Recolour a buffer in place and return the result as raw RGBA bytes.
///
/// Used by the parity fixture generator: the frontend runs its own port of this
/// maths for the live preview, and that port is only trustworthy if it produces
/// these exact bytes. Emitting the reference from the engine keeps it in one
/// place rather than as a hand-maintained table of expected values.
pub fn recolor_rgba_bytes(rgba: &mut image::RgbaImage, params: &RecolorParams) -> Vec<u8> {
    apply_to_rgba(rgba, params);
    rgba.as_raw().clone()
}

/// Apply `params` to an RGBA buffer in place.
pub fn apply_to_rgba(rgba: &mut image::RgbaImage, params: &RecolorParams) {
    match params.mode {
        RecolorMode::AllToOne => apply_all_to_one(rgba, params),
        RecolorMode::Replace => apply_rules(rgba, params),
    }
}

fn apply_rules(rgba: &mut image::RgbaImage, params: &RecolorParams) {
    // Drop disabled and duplicate rules up front: matching is the hot loop, so
    // every rule that survives here is one fewer comparison per pixel.
    let mut seen: HashSet<Rgb> = HashSet::new();
    let rules: Vec<(Lab, Rgb)> = params
        .rules
        .iter()
        .filter(|rule| seen.insert(rule.from))
        .map(|rule| (rule.from.oklab(), rule.to))
        .collect();

    if rules.is_empty() {
        return;
    }

    let max_distance = tolerance_to_distance(params.tolerance);

    for pixel in rgba.pixels_mut() {
        if pixel.0[3] == 0 {
            continue;
        }

        let source = Rgb::new(pixel.0[0], pixel.0[1], pixel.0[2]);
        let lab = source.oklab();

        // Nearest match wins, so overlapping rules cannot make the result
        // depend on the order the user happened to add them in.
        let mut best: Option<(f32, Rgb)> = None;
        for (rule_lab, target) in &rules {
            let distance = lab.distance(*rule_lab);
            if distance <= max_distance {
                match best {
                    Some((current, _)) if current <= distance => {}
                    _ => best = Some((distance, *target)),
                }
            }
        }

        if let Some((_, target)) = best {
            pixel.0[0] = target.r;
            pixel.0[1] = target.g;
            pixel.0[2] = target.b;
        }
    }
}

fn apply_all_to_one(rgba: &mut image::RgbaImage, params: &RecolorParams) {
    let target = params.all_to_one;
    let target_lab = target.oklab();

    if !params.preserve_shading {
        for pixel in rgba.pixels_mut() {
            if pixel.0[3] == 0 {
                continue;
            }
            pixel.0[0] = target.r;
            pixel.0[1] = target.g;
            pixel.0[2] = target.b;
        }
        return;
    }

    // Keep the source's lightness, take the target's hue and chroma. Flattening
    // a photo to one colour otherwise throws away all of its form: a bird
    // becomes a silhouette. Reconstructing in OKLab means the result stays in
    // gamut, so saturated targets do not clip to ugly flat patches.
    for pixel in rgba.pixels_mut() {
        if pixel.0[3] == 0 {
            continue;
        }
        let source = Rgb::new(pixel.0[0], pixel.0[1], pixel.0[2]).oklab();
        let shifted = lab_to_rgb(Lab {
            l: source.l.clamp(0.0, 1.0),
            a: target_lab.a,
            b: target_lab.b,
        });
        pixel.0[0] = shifted.r;
        pixel.0[1] = shifted.g;
        pixel.0[2] = shifted.b;
    }
}

/// Where a saved recolour went.
#[derive(Debug, Clone, PartialEq)]
pub struct RecolorResult {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    /// True when `params` changed nothing and the file was written as-is.
    pub unchanged: bool,
}

/// Recolour `input` and write the result to `output`.
///
/// The output format follows the output path's extension, so a `.jpg` input
/// saved as `.jpg` stays a JPEG. Formats that cannot store an alpha channel
/// are composited onto white first, matching what the convert engine does, so
/// transparent areas do not come back as black.
pub fn recolor_file(input: &Path, output: &Path, params: &RecolorParams) -> Result<RecolorResult> {
    info!("Recolouring {:?} -> {:?}", input, output);

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create destination directory: {:?}", parent))?;
        }
    }

    let source = crate::image_convert::load_image(input)
        .with_context(|| format!("Failed to open image for recolouring: {:?}", input))?;

    let width = source.width();
    let height = source.height();
    let recoloured = recolor(&source, params);

    let format = output_format(output).with_context(|| {
        format!(
            "Unsupported output format: {:?}",
            output.extension().and_then(|e| e.to_str()).unwrap_or("")
        )
    })?;

    let to_save = match format {
        ImageFormat::Jpeg | ImageFormat::Bmp => {
            DynamicImage::ImageRgb8(crate::image_convert::flatten_to_rgb(&recoloured, [255, 255, 255]))
        }
        _ => recoloured,
    };

    // Write to a temp file first, then move into place, so a failure part way
    // through never leaves a half-written file where the user expects a whole
    // one. See image_convert for why a rename may need a copy fallback.
    let temp_dir = std::env::temp_dir().join("wheel_temp");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp = temp_dir.join(format!(
        "recolor_{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        format.extensions_str()[0]
    ));

    to_save
        .save_with_format(&temp, format)
        .with_context(|| format!("Failed to encode recoloured image to {:?}", temp))?;

    if let Err(rename_err) = std::fs::rename(&temp, output) {
        std::fs::copy(&temp, output).with_context(|| {
            format!("Failed to save output to {:?}: {}", output, rename_err)
        })?;
        let _ = std::fs::remove_file(&temp);
    }

    Ok(RecolorResult {
        output_path: output.to_path_buf(),
        width,
        height,
        unchanged: params.is_noop(),
    })
}

/// Pick an encoder for the output path.
///
/// Falls back to PNG rather than erroring: the output path is chosen by the app
/// and only the extension can be unusable, so a sensible format beats failing a
/// job the user is waiting on.
fn output_format(output: &Path) -> Result<ImageFormat> {
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let chosen = match ext.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" | "jfif" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::WebP),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        "gif" => Some(ImageFormat::Gif),
        "ico" => Some(ImageFormat::Ico),
        "avif" => Some(ImageFormat::Avif),
        _ => None,
    };

    Ok(chosen.unwrap_or(ImageFormat::Png))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid(width: u32, height: u32, color: Rgb) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba([color.r, color.g, color.b, 255]))
    }

    fn px(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
        image.get_pixel(x, y).0
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(Rgb::from_hex("#E53E4A"), Some(Rgb::new(0xE5, 0x3E, 0x4A)));
        assert_eq!(Rgb::from_hex("e53e4a"), Some(Rgb::new(0xE5, 0x3E, 0x4A)));
        assert_eq!(Rgb::from_hex("  #E53E4A  "), Some(Rgb::new(0xE5, 0x3E, 0x4A)));
        assert_eq!(Rgb::from_hex("#abc"), Some(Rgb::new(0xAA, 0xBB, 0xCC)));
        assert_eq!(Rgb::from_hex("#ABC"), Some(Rgb::new(0xAA, 0xBB, 0xCC)));
        assert_eq!(Rgb::new(0xE5, 0x3E, 0x4A).to_hex(), "#E53E4A");
    }

    #[test]
    fn hex_rejects_nonsense() {
        for input in ["", "#", "#12", "#12345", "#ZZZZZZ", "rgb(1,2,3)", "#GGGGGG"] {
            assert_eq!(Rgb::from_hex(input), None, "accepted {input:?}");
        }
    }

    #[test]
    fn tolerance_zero_matches_only_the_exact_colour() {
        let blue = Rgb::new(0x1E, 0x4F, 0xBF);
        // One unit of blue away: a different colour, and tolerance 0 must not
        // touch it.
        let near_blue = Rgb::new(0x1E, 0x4F, 0xC0);

        let mut image = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, Rgba([blue.r, blue.g, blue.b, 255]));
        image.put_pixel(1, 0, Rgba([near_blue.r, near_blue.g, near_blue.b, 255]));

        let result = recolor(
            &DynamicImage::ImageRgba8(image),
            &RecolorParams {
                tolerance: 0,
                rules: vec![RecolorRule { from: blue, to: Rgb::new(255, 0, 0) }],
                ..Default::default()
            },
        );

        let out = result.to_rgba8();
        assert_eq!(px(&out, 0, 0), [255, 0, 0, 255], "the exact match was replaced");
        assert_eq!(
            px(&out, 1, 0),
            [near_blue.r, near_blue.g, near_blue.b, 255],
            "a near miss must survive at tolerance 0"
        );
    }

    #[test]
    fn tolerance_catches_near_shades_at_default_setting() {
        let base = Rgb::new(0x28, 0x76, 0xD2);
        let shade = Rgb::new(0x2C, 0x7D, 0xDB);
        let result = recolor(
            &DynamicImage::ImageRgba8(solid(1, 1, shade)),
            &RecolorParams {
                tolerance: default_tolerance(),
                rules: vec![RecolorRule { from: base, to: Rgb::new(255, 0, 0) }],
                ..Default::default()
            },
        );
        assert_eq!(result.to_rgba8().get_pixel(0, 0).0, [255, 0, 0, 255]);
    }

    #[test]
    fn replace_leaves_unmatched_colours_alone() {
        let blue = Rgb::new(0x1E, 0x4F, 0xBF);
        let yellow = Rgb::new(0xFD, 0xE0, 0x47);
        let result = recolor(
            &DynamicImage::ImageRgba8(solid(1, 1, yellow)),
            &RecolorParams {
                tolerance: default_tolerance(),
                rules: vec![RecolorRule { from: blue, to: Rgb::new(0xB9, 0x1C, 0x1C) }],
                ..Default::default()
            },
        );
        assert_eq!(result.to_rgba8().get_pixel(0, 0).0, [0xFD, 0xE0, 0x47, 255]);
    }

    #[test]
    fn replace_preserves_alpha() {
        let blue = Rgb::new(0x1E, 0x4F, 0xBF);
        let source = RgbaImage::from_pixel(1, 1, Rgba([blue.r, blue.g, blue.b, 77]));
        let result = recolor(
            &DynamicImage::ImageRgba8(source.clone()),
            &RecolorParams {
                tolerance: default_tolerance(),
                rules: vec![RecolorRule { from: blue, to: Rgb::new(255, 255, 255) }],
                ..Default::default()
            },
        );
        let out = result.to_rgba8();
        assert_eq!(out.get_pixel(0, 0).0, [255, 255, 255, 77]);
        // The input buffer must be untouched: callers preview by recolouring a
        // copy while still holding the original.
        assert_eq!(source.get_pixel(0, 0).0, [blue.r, blue.g, blue.b, 77]);
    }

    #[test]
    fn fully_transparent_pixels_are_never_touched() {
        // A real transparent PNG often keeps the RGB of whatever was painted
        // before the alpha was cleared. Recolouring that would resurrect colour
        // in a region the user believes is empty.
        let image = RgbaImage::from_pixel(1, 1, Rgba([0xFF, 0x00, 0x00, 0]));

        let result = recolor(
            &DynamicImage::ImageRgba8(image),
            &RecolorParams {
                mode: RecolorMode::AllToOne,
                all_to_one: Rgb::new(0, 255, 0),
                ..Default::default()
            },
        );
        assert_eq!(result.to_rgba8().get_pixel(0, 0).0, [0xFF, 0x00, 0x00, 0]);
    }

    #[test]
    fn nearest_rule_wins_regardless_of_order() {
        let base = Rgb::new(0x28, 0x76, 0xD2);
        let shade = Rgb::new(0x2C, 0x7D, 0xDB);
        let near = Rgb::new(0xB9, 0x1C, 0x1C);
        let far = Rgb::new(0xFB, 0x71, 0x85);

        let forward = vec![
            RecolorRule { from: base, to: near },
            RecolorRule { from: shade, to: far },
        ];
        let mut reversed = forward.clone();
        reversed.reverse();

        let params_for = |rules| RecolorParams {
            tolerance: default_tolerance(),
            rules,
            ..Default::default()
        };

        let a = recolor(&DynamicImage::ImageRgba8(solid(1, 1, shade)), &params_for(forward));
        let b = recolor(&DynamicImage::ImageRgba8(solid(1, 1, shade)), &params_for(reversed));

        assert_eq!(a.to_rgba8().get_pixel(0, 0).0, b.to_rgba8().get_pixel(0, 0).0);
        assert_eq!(a.to_rgba8().get_pixel(0, 0).0, [0xFB, 0x71, 0x85, 255]);
    }

    #[test]
    fn all_to_one_without_shading_flattens_exactly() {
        let target = Rgb::new(0xCB, 0xE7, 0x1F);
        let result = recolor(
            &DynamicImage::ImageRgba8(solid(4, 1, Rgb::new(0x11, 0x22, 0x33))),
            &RecolorParams {
                mode: RecolorMode::AllToOne,
                all_to_one: target,
                preserve_shading: false,
                ..Default::default()
            },
        );
        let out = result.to_rgba8();
        for x in 0..4 {
            assert_eq!(px(&out, x, 0), [target.r, target.g, target.b, 255]);
        }
    }

    #[test]
    fn all_to_one_keeps_shading_when_asked() {
        let mut image = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, Rgba([40, 40, 40, 255]));
        image.put_pixel(1, 0, Rgba([220, 220, 220, 255]));

        let result = recolor(
            &DynamicImage::ImageRgba8(image),
            &RecolorParams {
                mode: RecolorMode::AllToOne,
                all_to_one: Rgb::new(0xCB, 0xE7, 0x1F),
                preserve_shading: true,
                ..Default::default()
            },
        );

        let out = result.to_rgba8();
        let dark = out.get_pixel(0, 0).0;
        let light = out.get_pixel(1, 0).0;

        // Both pixels now carry the target hue, but the lightness difference
        // that makes the image readable survives.
        assert_ne!(dark, light, "shading was flattened away");
        let lightness = |p: [u8; 4]| f32::from(p[0]) + f32::from(p[1]) + f32::from(p[2]);
        assert!(lightness(light) > lightness(dark));
    }

    #[test]
    fn all_to_one_is_never_a_noop() {
        assert!(!RecolorParams {
            mode: RecolorMode::AllToOne,
            ..Default::default()
        }
        .is_noop());
        assert!(RecolorParams::default().is_noop());
    }

    #[test]
    fn no_rules_leaves_the_image_byte_identical() {
        let source = DynamicImage::ImageRgba8(solid(3, 3, Rgb::new(0x28, 0x76, 0xD2)));
        let before = source.to_rgba8();
        let after = recolor(&source, &RecolorParams::default()).to_rgba8();
        assert_eq!(before.as_raw(), after.as_raw());
    }

    #[test]
    fn extraction_is_exact_and_counts_every_distinct_colour() {
        let mut image = RgbaImage::from_pixel(4, 1, Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        image.put_pixel(1, 0, Rgba([10, 20, 30, 255]));
        // One unit apart from its neighbour: still two distinct colours.
        image.put_pixel(2, 0, Rgba([10, 20, 31, 255]));
        image.put_pixel(3, 0, Rgba([200, 200, 200, 255]));

        let found = extract_colors(&DynamicImage::ImageRgba8(image), 10);

        assert_eq!(found.total_unique, 3);
        assert!(!found.truncated);
        assert_eq!(found.visible_pixels, 4);
        assert_eq!(found.colors[0].color, Rgb::new(10, 20, 30));
        assert_eq!(found.colors[0].count, 2);
        assert_eq!(found.colors[1].color, Rgb::new(10, 20, 31));
        assert_eq!(found.colors[1].count, 1);
        assert_eq!(found.colors[2].color, Rgb::new(200, 200, 200));
        assert!((found.colors[0].coverage - 0.5).abs() < 1e-6);
    }

    #[test]
    fn extraction_ignores_transparent_pixels() {
        let mut image = RgbaImage::from_pixel(2, 1, Rgba([1, 2, 3, 255]));
        image.put_pixel(1, 0, Rgba([9, 9, 9, 0]));

        let found = extract_colors(&DynamicImage::ImageRgba8(image), 10);

        assert_eq!(found.total_unique, 1);
        assert_eq!(found.visible_pixels, 1);
        assert_eq!(found.colors[0].color, Rgb::new(1, 2, 3));
    }

    #[test]
    fn extraction_reports_truncation_instead_of_merging() {
        // 100 distinct greys, but only 10 asked for. The other 90 must be
        // reported as existing, not folded into the ten returned.
        let mut image = RgbaImage::new(100, 1);
        for x in 0..100u32 {
            let v = x as u8 * 2;
            image.put_pixel(x, 0, Rgba([v, v, v, 255]));
        }

        let found = extract_colors(&DynamicImage::ImageRgba8(image), 10);

        assert_eq!(found.total_unique, 100);
        assert_eq!(found.colors.len(), 10);
        assert!(found.truncated);
        // Every returned colour is still exactly one of the original values.
        for entry in &found.colors {
            assert_eq!(entry.color.r % 2, 0);
        }
    }

    #[test]
    fn extraction_of_a_flat_image_is_a_single_colour() {
        let found = extract_colors(
            &DynamicImage::ImageRgba8(solid(16, 16, Rgb::new(0x1E, 0x4F, 0xBF))),
            100,
        );
        assert_eq!(found.total_unique, 1);
        assert_eq!(found.colors[0].color, Rgb::new(0x1E, 0x4F, 0xBF));
        assert_eq!(found.colors[0].count, 256);
    }

    #[test]
    fn a_single_flat_colour_with_antialiased_edges_is_still_one_colour() {
        // The regression this fixes: a rasterised SVG arrives premultiplied and is
        // un-premultiplied back to straight alpha, and that round trip is lossy.
        // A one-colour logo came back as six near-identical oranges, because the
        // edge pixels at different alphas rounded to different bytes.
        //
        // The bytes below are the real ones: `#FF5200` at each of those alphas,
        // passed through the same premultiply and unpremultiply the decoder does.
        let orange = Rgb::new(0xFF, 0x52, 0x00);
        let mut image = RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]));
        let mut index = 0;
        for alpha in [255u8, 207, 191, 176, 160, 144, 128, 96, 80, 64, 48, 32] {
            let a32 = u32::from(alpha);
            let premul = |v: u8| (((v as u32 * a32 + 127) / 255) as u8).min(255);
            let un = |v: u8| {
                (((v as u32) * 255 + a32 / 2) / a32).min(255) as u8
            };
            image.put_pixel(
                index % 16,
                index / 16,
                Rgba([
                    un(premul(orange.r)),
                    un(premul(orange.g)),
                    un(premul(orange.b)),
                    alpha,
                ]),
            );
            index += 1;
        }

        let found = extract_colors(&DynamicImage::ImageRgba8(image), 50);

        assert_eq!(
            found.total_unique, 1,
            "one paint must be reported as one colour, got {:?}",
            found
                .colors
                .iter()
                .map(|c| c.color.to_hex())
                .collect::<Vec<_>>()
        );
        // Reported as the opaque colour, since that is the one the user actually
        // painted - not one of the edge roundings.
        assert_eq!(found.colors[0].color, orange);
    }

    #[test]
    fn a_genuine_colour_in_a_translucent_region_is_still_reported() {
        // The fix must not throw away colours that only exist at partial alpha -
        // that is the other half of what extraction is for. This pixel has no
        // opaque counterpart anywhere in the image, so it has to survive as its
        // own entry.
        let mut image = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        image.put_pixel(0, 0, Rgba([0x1E, 0x4F, 0xBF, 255]));
        image.put_pixel(1, 0, Rgba([0x9B, 0x1C, 0x1C, 128]));

        let found = extract_colors(&DynamicImage::ImageRgba8(image), 10);

        let hexes: Vec<_> = found.colors.iter().map(|c| c.color.to_hex()).collect();
        assert!(
            hexes.contains(&"#1E4FBF".to_string()),
            "the opaque colour must be listed, got {hexes:?}"
        );
        assert!(
            hexes.iter().any(|h| h.starts_with("#9B1C1C")),
            "a translucent-only colour must survive, got {hexes:?}"
        );
    }

    #[test]
    fn opaque_pixels_are_never_merged_with_each_other() {
        // Only translucent pixels are attributed to a nearby opaque colour. Two
        // opaque colours one unit apart are two real colours and must stay two,
        // which is what keeps a gradient's exactness intact.
        let mut image = RgbaImage::from_pixel(4, 1, Rgba([0, 0, 0, 0]));
        image.put_pixel(0, 0, Rgba([100, 100, 100, 255]));
        image.put_pixel(1, 0, Rgba([101, 100, 100, 255]));
        image.put_pixel(2, 0, Rgba([100, 100, 100, 255]));
        image.put_pixel(3, 0, Rgba([101, 100, 100, 255]));

        let found = extract_colors(&DynamicImage::ImageRgba8(image), 10);

        assert_eq!(found.total_unique, 2, "one-unit-apart opaque colours stay distinct");
    }

    #[test]
    fn transparent_flat_image_has_no_colours_at_all() {
        let found = extract_colors(&DynamicImage::ImageRgba8(RgbaImage::new(8, 8)), 10);
        assert_eq!(found.total_unique, 0);
        assert_eq!(found.visible_pixels, 0);
        assert!(found.colors.is_empty());
        assert!(!found.truncated);
    }

    #[test]
    fn saving_picks_the_encoder_from_the_output_extension() {
        let dir = std::env::temp_dir().join("wheel_recolor_format_test");
        let _ = std::fs::create_dir_all(&dir);
        let source = dir.join("in.png");
        DynamicImage::ImageRgba8(solid(2, 2, Rgb::new(0x28, 0x76, 0xD2)))
            .save(&source)
            .unwrap();

        for (name, expected) in [
            ("out.png", ImageFormat::Png),
            ("out.jpg", ImageFormat::Jpeg),
            ("out.webp", ImageFormat::WebP),
            ("out.bmp", ImageFormat::Bmp),
            ("out.tif", ImageFormat::Tiff),
        ] {
            let output = dir.join(name);
            recolor_file(
                &source,
                &output,
                &RecolorParams {
                    rules: vec![RecolorRule {
                        from: Rgb::new(0x28, 0x76, 0xD2),
                        to: Rgb::new(0xB9, 0x1C, 0x1C),
                    }],
                    ..Default::default()
                },
            )
            .unwrap();

            let reopened = image::open(&output).unwrap();
            let centre = reopened.to_rgba8().get_pixel(1, 1).0;
            // JPEG is lossy, so allow a small drift; the others must be exact.
            let drift = i32::from(centre[0]) - 0xB9;
            assert!(
                drift.abs() < (if expected == ImageFormat::Jpeg { 24 } else { 2 }),
                "{name} centre was {center:?}",
                center = centre
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_to_jpeg_flattens_transparency_onto_white() {
        let dir = std::env::temp_dir().join("wheel_recolor_jpeg_alpha");
        let _ = std::fs::create_dir_all(&dir);
        let source = dir.join("in.png");

        let mut image = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        image.put_pixel(1, 1, Rgba([0x28, 0x76, 0xD2, 255]));
        DynamicImage::ImageRgba8(image).save(&source).unwrap();

        let output = dir.join("out.jpg");
        let result = recolor_file(
            &source,
            &output,
            &RecolorParams {
                rules: vec![RecolorRule {
                    from: Rgb::new(0x28, 0x76, 0xD2),
                    to: Rgb::new(0xB9, 0x1C, 0x1C),
                }],
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.width, 4);
        assert_eq!(result.height, 4);
        assert!(!result.unchanged);

        let reopened = image::open(&output).unwrap().to_rgb8();
        let transparent_area = reopened.get_pixel(0, 0).0;
        assert!(
            transparent_area[0] > 240 && transparent_area[1] > 240 && transparent_area[2] > 240,
            "transparent area came back as {transparent_area:?}, expected near-white"
        );
    }

    #[test]
    fn saving_to_an_unknown_extension_falls_back_instead_of_failing() {
        let dir = std::env::temp_dir().join("wheel_recolor_unknown_ext");
        let _ = std::fs::create_dir_all(&dir);
        let source = dir.join("in.png");
        DynamicImage::ImageRgba8(solid(2, 2, Rgb::new(0x28, 0x76, 0xD2)))
            .save(&source)
            .unwrap();

        let output = dir.join("out.unknown");
        recolor_file(&source, &output, &RecolorParams::default()).unwrap();

        assert!(output.exists(), "the file should still be written");
        // `image::open` cannot sniff a format from ".unknown", so decode the
        // way the fallback encoder actually wrote it.
        let reopened = image::load_from_memory_with_format(
            &std::fs::read(&output).unwrap(),
            ImageFormat::Png,
        )
        .unwrap();
        assert_eq!(reopened.width(), 2);
    }

    #[test]
    fn saving_reports_a_noop_rather_than_failing() {
        let dir = std::env::temp_dir().join("wheel_recolor_noop");
        let _ = std::fs::create_dir_all(&dir);
        let source = dir.join("in.png");
        DynamicImage::ImageRgba8(solid(2, 2, Rgb::new(0x28, 0x76, 0xD2)))
            .save(&source)
            .unwrap();

        let result = recolor_file(&source, &dir.join("out.png"), &RecolorParams::default()).unwrap();
        assert!(result.unchanged);
        assert_eq!(
            image::open(&result.output_path).unwrap().to_rgba8().get_pixel(0, 0).0,
            [0x28, 0x76, 0xD2, 255]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_creates_missing_destination_directories() {
        let dir = std::env::temp_dir().join("wheel_recolor_mkdir");
        let _ = std::fs::remove_dir_all(&dir);
        let source = dir.join("in.png");
        std::fs::create_dir_all(&dir).unwrap();
        DynamicImage::ImageRgba8(solid(2, 2, Rgb::new(0x28, 0x76, 0xD2)))
            .save(&source)
            .unwrap();

        let output = dir.join("nested").join("deeper").join("out.png");
        recolor_file(&source, &output, &RecolorParams::default()).unwrap();

        assert!(output.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tolerance_scale_is_monotonic() {
        let mut previous = -1.0;
        for t in 0..=100u8 {
            let d = tolerance_to_distance(t);
            assert!(d > previous, "tolerance {t} did not widen the radius");
            previous = d;
        }
        assert!(tolerance_to_distance(0) == 0.0);
        // The useful range sits in the low half, not the top.
        assert!(tolerance_to_distance(30) < 0.1);
    }

    // ---- grouping the extracted list -------------------------------------

    /// A flat colour with a cloud of near neighbours, like a lossy WebP of an
    /// icon, plus one clearly separate colour so grouping has something to not
    /// merge.
    fn clustered_image() -> DynamicImage {
        let seed = Rgb::new(0x2E, 0xAB, 0xB8);
        let accents = [
            Rgb::new(0x35, 0xB0, 0xBD),
            Rgb::new(0x39, 0xB3, 0xC0),
            Rgb::new(0x36, 0xB1, 0xBE),
            Rgb::new(0x32, 0xAE, 0xBB),
            Rgb::new(0x31, 0xAD, 0xBA),
            Rgb::new(0x3D, 0xB6, 0xC3),
            Rgb::new(0x50, 0xBC, 0xC7),
            Rgb::new(0xE0, 0x8D, 0xBF),
        ];
        let pink = Rgb::new(0xE0, 0x8D, 0xBF);

        // Built as a buffer rather than through `to_rgba8`, which copies.
        let mut buffer = image::RgbaImage::new(24, 24);
        for (x, y, pixel) in buffer.enumerate_pixels_mut() {
            let color = if x < 18 && y < 18 {
                // The seed has to genuinely dominate, because a group is led by
                // its most common member.
                if (x * 7 + y * 13) % 10 == 0 {
                    accents[((x + y) % 7 + 1) as usize]
                } else {
                    seed
                }
            } else {
                pink
            };
            *pixel = image::Rgba([color.r, color.g, color.b, 255]);
        }
        DynamicImage::ImageRgba8(buffer)
    }

    #[test]
    fn tolerance_zero_groups_nothing() {
        // Exact is what the user asked for, and it has to be reachable.
        let img = clustered_image();
        let exact = extract_colors(&img, 4096);
        let grouped = group_colors(&exact.colors, 0);

        assert_eq!(
            grouped.len(),
            exact.colors.len(),
            "tolerance 0 must change nothing"
        );
        assert!(grouped.iter().all(|g| g.merged == 1));
    }

    #[test]
    fn grouping_merges_the_cluster_and_keeps_the_accent() {
        let img = clustered_image();
        let exact = extract_colors(&img, 4096);
        let grouped = group_colors(&exact.colors, default_tolerance());

        assert!(
            grouped.len() < exact.colors.len(),
            "tolerance 30 grouped nothing: {grouped:?}"
        );

        // The cluster becomes one swatch; the pink stays on its own because it
        // is nowhere near the teal.
        let teal = grouped
            .iter()
            .find(|g| g.color == Rgb::new(0x2E, 0xAB, 0xB8))
            .expect("the most common colour leads its own group");
        assert!(
            teal.merged >= 7,
            "expected the near teals merged, got {}",
            teal.merged
        );

        let pink = grouped
            .iter()
            .find(|g| g.color == Rgb::new(0xE0, 0x8D, 0xBF))
            .expect("the pink must survive as its own group");
        assert_eq!(pink.merged, 1, "the pink has no near neighbours");
    }

    #[test]
    fn a_group_repaint_exactly_what_the_swatch_says_it_will() {
        // The whole point of grouping: what the list shows has to be what the
        // edit does. If these drift apart, the list is decoration.
        let img = clustered_image();
        let exact = extract_colors(&img, 4096);
        let tolerance = default_tolerance();
        let grouped = group_colors(&exact.colors, tolerance);

        let target = Rgb::new(255, 0, 0);
        let teal_group = grouped
            .iter()
            .find(|g| g.color == Rgb::new(0x2E, 0xAB, 0xB8))
            .expect("teal group");

        let mut after = img.to_rgba8();
        apply_rules(&mut after, &RecolorParams {
            rules: vec![RecolorRule {
                from: teal_group.color,
                to: target,
            }],
            tolerance,
            ..Default::default()
        });

        // Which of the *original* colours the rule actually touched. Comparing
        // the result to the group's members is the whole claim.
        let before = img.to_rgba8();
        let changed: std::collections::HashSet<[u8; 3]> = before
            .pixels()
            .zip(after.pixels())
            .filter(|(b, a)| b.0 != a.0)
            .map(|(b, _)| [b.0[0], b.0[1], b.0[2]])
            .collect();

        let expected: std::collections::HashSet<[u8; 3]> = exact
            .colors
            .iter()
            .filter(|c| {
                teal_group
                    .color
                    .oklab()
                    .distance(c.color.oklab())
                    <= tolerance_to_distance(tolerance)
            })
            .map(|c| [c.color.r, c.color.g, c.color.b])
            .collect();

        assert_eq!(
            changed, expected,
            "the swatch claimed {} colours but the rule repainted {}",
            teal_group.merged,
            changed.len()
        );
        assert!(!changed.contains(&[0xE0, 0x8D, 0xBF]), "the pink moved");
    }

    #[test]
    fn grouping_is_deterministic() {
        // Hash iteration order must never reach the user: an earlier version of
        // this returned 20, then 21, then 22 swatches for the same file.
        let img = clustered_image();
        let exact = extract_colors(&img, 4096);

        for tolerance in [0u8, 10, 20, 30, 40, 50] {
            let first = group_colors(&exact.colors, tolerance);
            for _ in 0..25 {
                assert_eq!(
                    group_colors(&exact.colors, tolerance),
                    first,
                    "grouping drifted at tolerance {tolerance}"
                );
            }
        }
    }

    #[test]
    fn grouping_survives_input_shuffled_by_hash_order() {
        // Same colours, arbitrary order: the groups must come out identical.
        let img = clustered_image();
        let exact = extract_colors(&img, 4096);
        let expected = group_colors(&exact.colors, default_tolerance());

        let mut shuffled = exact.colors.clone();
        shuffled.reverse();
        assert_eq!(
            group_colors(&shuffled, default_tolerance()),
            expected,
            "the result depends on the order colours arrive in"
        );
    }

    #[test]
    fn a_colour_between_two_seeds_joins_the_nearer_one() {
        // Matches "nearest rule wins" in the recolour itself. First-match would
        // put this in the wrong swatch and contradict the result.
        let near_a = Rgb::new(0x28, 0x76, 0xD2);
        let near_b = Rgb::new(0x2C, 0x7D, 0xDB);
        let between = Rgb::new(0x2A, 0x7A, 0xD6);

        let colors: Vec<ExtractedColor> = [near_a, near_b, between]
            .into_iter()
            .enumerate()
            .map(|(i, color)| ExtractedColor {
                color,
                count: 100 - i as u64,
                coverage: 0.0,
            })
            .collect();

        let grouped = group_colors(&colors, default_tolerance());
        assert_eq!(grouped.len(), 1, "everything here is close: {grouped:?}");

        // And when the two seeds are far enough apart to stay separate.
        let far = Rgb::new(0xB9, 0x1C, 0x1C);
        let colors: Vec<ExtractedColor> = [near_a, far, between]
            .into_iter()
            .enumerate()
            .map(|(i, color)| ExtractedColor {
                color,
                count: 100 - i as u64,
                coverage: 0.0,
            })
            .collect();

        let grouped = group_colors(&colors, default_tolerance());
        assert_eq!(grouped.len(), 2, "got {grouped:?}");
        assert_eq!(grouped.len(), 2);
        let with_between = grouped
            .iter()
            .find(|g| g.merged == 2)
            .expect("the middle colour joins one of them");
        assert_eq!(with_between.color, near_a, "the nearer seed takes it");
    }

    #[test]
    fn every_colour_is_covered_and_nothing_escapes_a_swatch() {
        // Groups overlap by design - each one is a rule's radius, not a share of
        // the list - so the counts deliberately do not sum. What has to hold is
        // that they cover everything, and that no swatch claims more than the
        // whole image.
        let img = clustered_image();
        let exact = extract_colors(&img, 4096);

        for tolerance in [0u8, 20, 30, 50] {
            let radius = tolerance_to_distance(tolerance);
            let grouped = group_colors(&exact.colors, tolerance);

            // Every colour lands in at least one swatch.
            for entry in &exact.colors {
                assert!(
                    grouped
                        .iter()
                        .any(|g| g.color.oklab().distance(entry.color.oklab()) <= radius),
                    "tolerance {tolerance}: {} was left uncovered",
                    entry.color.to_hex()
                );
            }

            for group in &grouped {
                assert!(
                    (0.0..=1.0).contains(&group.coverage),
                    "coverage {} is not a share",
                    group.coverage
                );
                assert!(group.count <= exact.colors.iter().map(|c| c.count).sum::<u64>());

                // `merged` is the definition of the swatch: the listed colours
                // inside its radius. Pinned here because it is the number the
                // window puts on the swatch.
                let inside = exact
                    .colors
                    .iter()
                    .filter(|entry| {
                        group.color.oklab().distance(entry.color.oklab()) <= radius
                    })
                    .count() as u64;
                assert_eq!(
                    group.merged, inside,
                    "tolerance {tolerance}: {} claims {} but {} are in range",
                    group.color.to_hex(),
                    group.merged,
                    inside
                );
            }
        }
    }

    #[test]
    fn grouping_an_empty_list_is_empty() {
        assert!(group_colors(&[], 30).is_empty());
    }

    #[test]
    fn oklab_distance_ranks_visually_adjacent_colours_closer() {
        let blue = Rgb::new(0x1E, 0x4F, 0xBF).oklab();
        let near_blue = Rgb::new(0x28, 0x76, 0xD2).oklab();
        let red = Rgb::new(0xB9, 0x1C, 0x1C).oklab();

        assert!(blue.distance(near_blue) < blue.distance(red));
    }

    #[test]
    fn rules_survive_a_json_round_trip() {
        // The UI sends these across the Tauri bridge as JSON, so the wire format
        // has to match what the frontend expects.
        let params = RecolorParams {
            mode: RecolorMode::AllToOne,
            rules: vec![RecolorRule {
                from: Rgb::new(0x1E, 0x4F, 0xBF),
                to: Rgb::new(0xE5, 0x3E, 0x4A),
            }],
            all_to_one: Rgb::new(203, 231, 31),
            tolerance: 42,
            preserve_shading: false,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"all_to_one\":\"#CBE71F\""), "got {json}");
        assert!(json.contains("\"from\":\"#1E4FBF\""), "got {json}");
        assert!(json.contains("\"mode\":\"all_to_one\""), "got {json}");

        let parsed: RecolorParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, params);
    }

    #[test]
    fn a_colour_that_is_not_a_hex_string_is_a_parse_error() {
        // Better to reject the whole payload than to silently substitute a
        // colour the user did not ask for.
        let parsed = serde_json::from_str::<RecolorParams>(
            r##"{ "mode": "replace", "rules": [{ "from": "chartreuse", "to": "#000000" }] }"##,
        );
        assert!(parsed.is_err());
    }

    #[test]
    fn all_to_one_defaults_to_a_flat_fill() {
        // The UI's default and this one have to agree, or the preview shows a
        // flat silhouette while the saved file keeps its shading.
        assert!(
            !RecolorParams::default().preserve_shading,
            "Preserve shading must be off by default"
        );

        // And an omitted key must mean the same thing.
        let parsed: RecolorParams =
            serde_json::from_str(r#"{ "mode": "all_to_one" }"#).unwrap();
        assert!(!parsed.preserve_shading);
    }

    #[test]
    fn omitted_params_fall_back_to_defaults() {
        // A minimal payload must mean "replace mode, nothing selected" rather
        // than failing to parse.
        let parsed: RecolorParams = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, RecolorParams::default());
        assert!(parsed.is_noop());
    }
}
