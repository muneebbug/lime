//! Image compression.
//!
//! This module owns the vocabulary the rest of the app speaks: presets, a
//! maximum size, a target file size, and per-format overrides. `libcaesium` is
//! an implementation detail that appears nowhere else, so replacing the backend
//! means editing this file and nothing else.
//!
//! Two things are deliberately not exposed:
//!
//! * **GIF.** Libcaesium's GIF support is partial and its own documentation
//!   admits GIF optimisation is "currently not supported". Re-encoding an
//!   animation risks flattening it to a single frame, which is data loss rather
//!   than compression, so no GIF reaches this module at all.
//! * **Vector and already-packed formats.** SVG would have to be rasterised to
//!   shrink, which is the same mistake as trying to trim one. ICO is already
//!   packed. Neither is worth compressing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How hard to push. The names are the user's, not the library's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressPreset {
    /// Visually indistinguishable in most cases. The default, because a
    /// compression tool that visibly degrades images is not a tool anyone
    /// trusts to run twice.
    Balanced,
    /// Smaller, with quality loss you may notice if you look for it.
    Strong,
    /// Every lossless gain plus the slowest encoders. Minutes on a large PNG.
    Maximal,
}

impl Default for CompressPreset {
    fn default() -> Self {
        CompressPreset::Balanced
    }
}

/// How to subsample JPEG chroma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChromaSubsampling {
    /// No subsampling. Largest, best for screenshots and flat art.
    C444,
    C422,
    /// The web default.
    C420,
    C411,
    /// The original JPEG default. Smallest, softest on edges.
    C410,
}

impl ChromaSubsampling {
    fn to_caesium(self) -> caesium::parameters::ChromaSubsampling {
        use caesium::parameters::ChromaSubsampling as Cs;
        match self {
            ChromaSubsampling::C444 => Cs::CS444,
            ChromaSubsampling::C422 => Cs::CS422,
            ChromaSubsampling::C420 => Cs::CS420,
            ChromaSubsampling::C411 => Cs::CS411,
            // The library's own default, and its documented choice.
            ChromaSubsampling::C410 => Cs::Auto,
        }
    }
}

/// How a TIFF should be packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TiffCompression {
    /// Store raw samples. Enormous, and only useful when nothing else is allowed.
    None,
    Lzw,
    Deflate,
    Packbits,
}

impl TiffCompression {
    fn to_caesium(self) -> caesium::parameters::TiffCompression {
        use caesium::parameters::TiffCompression as Cs;
        match self {
            TiffCompression::None => Cs::Uncompressed,
            TiffCompression::Lzw => Cs::Lzw,
            TiffCompression::Deflate => Cs::Deflate,
            TiffCompression::Packbits => Cs::Packbits,
        }
    }
}

/// How hard to squeeze a TIFF's deflate stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TiffDeflateLevel {
    Fast,
    Balanced,
    Best,
}

impl TiffDeflateLevel {
    fn to_caesium(self) -> caesium::parameters::TiffDeflateLevel {
        use caesium::parameters::TiffDeflateLevel as Cs;
        match self {
            TiffDeflateLevel::Fast => Cs::Fast,
            TiffDeflateLevel::Balanced => Cs::Balanced,
            TiffDeflateLevel::Best => Cs::Best,
        }
    }
}

/// Per-format overrides. `None` means "whatever the preset chose".
///
/// Every field is optional on purpose: a preset is a coherent set of choices,
/// and letting the user change one knob without the others leaves them with a
/// combination the preset never described and cannot predict.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdvancedOptions {
    /// 0-100. Overrides the preset for JPEG and PNG.
    pub quality: Option<u8>,
    pub chroma_subsampling: Option<ChromaSubsampling>,
    /// Progressive JPEG. Smaller at the same quality, and renders progressively
    /// over a slow connection.
    pub progressive: Option<bool>,
    /// Re-compress the JPEG losslessly: rebuild the Huffman tables without
    /// touching a single pixel. Typically 10% off, and the image is bit-for-bit
    /// the same.
    ///
    /// Named for what it does rather than after the library's `jpeg_optimize`.
    /// That flag reads like mozjpeg's trellis quantisation, which is the big
    /// lossless gain, but it is not: libcaesium routes it to a `jpegtran`-style
    /// path that never calls `jpeg_set_quality`. Turning it on therefore
    /// *discards* the quality setting - measured, every quality produced the same
    /// 751,503-byte file - so the presets leave it off and it is opt-in here.
    pub lossless_optimize: Option<bool>,
    /// On: lossless PNG optimisation. Off: quantise to a 256-colour palette.
    ///
    /// Not a strength dial. Libcaesium's `png.optimize` picks between a lossless
    /// and a lossy path rather than trying harder at one, and the lossy side is
    /// imagequant. Measured on a real 1.43 MB PNG, turning it *off* took the file
    /// from 1.28 MB to 0.62 MB - a far bigger win than anything lossless offers,
    /// at the cost of reducing the image to 256 colours.
    pub optimize_png: Option<bool>,
    /// Zopfli. Measured on that same file: 75 seconds for 0.7 percentage points.
    /// Genuinely the smallest deflate stream available, and priced accordingly.
    /// Never in a preset, only here.
    pub force_zopfli: Option<bool>,
    /// Re-encode WebP losslessly. Note that libcaesium warns this usually makes
    /// the file *bigger*, since WebP already compresses well.
    pub webp_lossless: Option<bool>,
    pub tiff_compression: Option<TiffCompression>,
    /// Only meaningful with Deflate.
    pub tiff_deflate_level: Option<TiffDeflateLevel>,
    /// Keep the ICC colour profile even when metadata is dropped. Dropping it
    /// silently changes how the image looks on a wide-gamut display.
    pub preserve_icc: Option<bool>,
}

/// Everything the window can ask for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompressParams {
    pub preset: CompressPreset,
    /// Longest edge the output may have. `None` keeps the original size.
    pub max_dimension: Option<u32>,
    /// Try to land at or under this many bytes.
    pub target_size: Option<u64>,
    /// Keep EXIF and friends. Off by default: phone photos routinely carry tens
    /// of kilobytes nobody looks at, and compression is usually for sharing.
    pub keep_metadata: bool,
    /// Keep the orientation tag regardless of `keep_metadata`. Separate because
    /// getting this wrong rotates the image, and that is worse than a big file.
    pub keep_rotation: bool,
    pub advanced: AdvancedOptions,
}

impl Default for CompressParams {
    fn default() -> Self {
        CompressParams {
            preset: CompressPreset::Balanced,
            max_dimension: None,
            target_size: None,
            keep_metadata: false,
            keep_rotation: true,
            advanced: AdvancedOptions::default(),
        }
    }
}

/// What a run did, in the words the window shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompressResult {
    pub output_path: PathBuf,
    pub original_size: u64,
    pub output_size: u64,
    /// True when the file could not be made any smaller and was left alone.
    pub unchanged: bool,
    /// True when a target was asked for and not reached. Never reported as
    /// success: "closest we got" is the honest answer, and pretending otherwise
    /// would make the feature a lie.
    pub target_missed: bool,
}

impl CompressResult {
    /// Bytes saved, or zero if the file grew.
    pub fn saved(&self) -> u64 {
        self.original_size.saturating_sub(self.output_size)
    }

    /// Share of the original size removed, 0.0 to 1.0.
    pub fn ratio(&self) -> f32 {
        if self.original_size == 0 {
            return 0.0;
        }
        self.saved() as f32 / self.original_size as f32
    }
}

/// Formats this module will compress.
///
/// Deliberately narrower than what the app can open. See the module docs.
///
/// `tif` sits beside `tiff` because they are the same format and a user who
/// saved with one spelling should not lose the tool. Every other tool in the app
/// accepts both.
pub const COMPRESSIBLE: &[&str] = &["jpg", "jpeg", "png", "webp", "tiff", "tif"];

pub fn is_compressible(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| COMPRESSIBLE.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Reject inputs that are not actually images, before any encoder sees them.
///
/// A decode failure inside a codec is the kind of thing that can abort the
/// process under `panic = "abort"`, which this build uses, so the cheap checks
/// happen here where a normal `Result` can carry them.
pub fn validate_input(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("Not a file: {}", path.display()));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !COMPRESSIBLE.contains(&ext.as_str()) {
        return Err(format!("{} is not a format worth compressing.", path.display()));
    }

    let head = std::fs::read(path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    // A file can be empty, truncated, or not an image at all despite its name.
    if head.is_empty() {
        return Err(format!("{} is empty.", path.display()));
    }
    if !looks_like_an_image(&head) {
        return Err(format!(
            "{} is not a readable image, whatever its extension says.",
            path.display()
        ));
    }
    Ok(())
}

/// Magic-number check for the four supported containers.
///
/// Enough to tell a real image from a text file that happens to be called
/// `.jpg`, which is what a mis-selected download usually is.
fn looks_like_an_image(head: &[u8]) -> bool {
    const JPEG: [u8; 3] = [0xFF, 0xD8, 0xFF];
    const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    if head.starts_with(&JPEG) || head.starts_with(&PNG) {
        return true;
    }
    // RIFF....WEBP
    if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        return true;
    }
    // II*\0 or MM\0*
    if head.len() >= 4 {
        let little = &head[..4] == [0x49, 0x49, 0x2A, 0x00];
        let big = &head[..4] == [0x4D, 0x4D, 0x00, 0x2A];
        if little || big {
            return true;
        }
    }
    false
}

/// Compress `input` to `output` in place of a rename, so a failure never leaves
/// a half-written file where the user's picture was.
pub fn compress_file(
    input: &Path,
    output: &Path,
    params: &CompressParams,
) -> Result<CompressResult, String> {
    validate_input(input)?;

    let original_size = std::fs::metadata(input)
        .map_err(|e| format!("Could not stat {}: {e}", input.display()))?
        .len();

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }

    let stage = stage_path(output);
    // Anything already at the staging path is a leftover from a run that died.
    let _ = std::fs::remove_file(&stage);

    run(input, &stage, params)?;

    let produced = std::fs::metadata(&stage).map(|m| m.len()).unwrap_or(0);

    if produced == 0 {
        let _ = std::fs::remove_file(&stage);
        return Err(format!("Compression produced no output for {}.", input.display()));
    }

    // Compressing an already-optimised file can make it marginally larger.
    // Handing back a bigger file with the word "compressed" on it is worse than
    // doing nothing, so the original is kept and the difference is reported.
    if produced >= original_size {
        let _ = std::fs::remove_file(&stage);
        return Ok(CompressResult {
            output_path: output.to_path_buf(),
            original_size,
            output_size: original_size,
            unchanged: true,
            target_missed: params.target_size.is_some_and(|t| original_size > t),
        });
    }

    if std::fs::rename(&stage, output).is_err() {
        // A rename across volumes fails; a copy does not.
        std::fs::copy(&stage, output)
            .map_err(|e| format!("Could not write {}: {e}", output.display()))?;
        let _ = std::fs::remove_file(&stage);
    }

    Ok(CompressResult {
        output_path: output.to_path_buf(),
        original_size,
        output_size: produced,
        unchanged: false,
        target_missed: params.target_size.is_some_and(|t| produced > t),
    })
}

/// A neighbouring temp name, so the final move is atomic on the same volume.
fn stage_path(output: &Path) -> PathBuf {
    let name = output
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "out".to_string());
    let stage = format!(".{name}.compressing");
    match output.parent() {
        Some(parent) => parent.join(stage),
        None => PathBuf::from(stage),
    }
}

fn run(input: &Path, staged: &Path, params: &CompressParams) -> Result<(), String> {
    let mut cs = build_parameters(params);

    let input_s = path_arg(input);
    let staged_s = path_arg(staged);

    if let Some(target) = params.target_size {
        // `true` asks for the smallest result that fits rather than the first
        // one that does, which is slower but is what "compress to this size"
        // means to someone aiming it at an upload limit.
        let target = usize::try_from(target).map_err(|_| {
            format!("{} is larger than this build can express.", input.display())
        })?;
        caesium::compress_to_size(input_s.clone(), staged_s.clone(), &mut cs, target, true)
            .map_err(|e| describe(&e, input))?;
    } else {
        caesium::compress(input_s, staged_s, &cs).map_err(|e| describe(&e, input))?;
    }
    Ok(())
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// Libcaesium's errors are opaque enough that a bare message would be useless.
fn describe(error: &caesium::error::CaesiumError, input: &Path) -> String {
    format!("Could not compress {}: {error}", input.display())
}

fn build_parameters(params: &CompressParams) -> caesium::parameters::CSParameters {
    let preset = preset_values(params.preset);
    let advanced = &params.advanced;

    let mut cs = caesium::parameters::CSParameters::new();
    cs.keep_metadata = params.keep_metadata;
    cs.keep_rotation = params.keep_rotation;

    cs.jpeg.quality = pick(advanced.quality, preset.jpeg_quality);
    cs.jpeg.chroma_subsampling = advanced
        .chroma_subsampling
        .map(ChromaSubsampling::to_caesium)
        .unwrap_or(preset.jpeg_chroma_subsampling);
    cs.jpeg.progressive = advanced.progressive.unwrap_or(preset.jpeg_progressive);
    cs.jpeg.optimize = advanced.lossless_optimize.unwrap_or(preset.lossless_optimize);
    // Defaults to true upstream, which would keep a profile the rest of the
    // settings throw away. Tied to the metadata decision instead.
    cs.jpeg.preserve_icc = advanced
        .preserve_icc
        .unwrap_or(params.keep_metadata);

    cs.png.quality = pick(advanced.quality, preset.png_quality);
    cs.png.optimization_level = preset.png_optimization_level;
    cs.png.force_zopfli = advanced.force_zopfli.unwrap_or(preset.png_force_zopfli);
    cs.png.optimize = advanced.optimize_png.unwrap_or(preset.png_optimize);

    cs.webp.quality = pick(advanced.quality, preset.webp_quality);
    cs.webp.lossless = advanced.webp_lossless.unwrap_or(false);

    cs.tiff.algorithm = advanced
        .tiff_compression
        .map(TiffCompression::to_caesium)
        .unwrap_or(preset.tiff_compression);
    cs.tiff.deflate_level = advanced
        .tiff_deflate_level
        .map(TiffDeflateLevel::to_caesium)
        .unwrap_or(caesium::parameters::TiffDeflateLevel::Balanced);

    // Zero means "keep the original size" to the library.
    cs.width = params.max_dimension.unwrap_or(0);
    cs.height = 0;

    cs
}

fn pick(override_value: Option<u8>, preset_value: u32) -> u32 {
    override_value.map_or(preset_value, u32::from)
}

/// The numbers behind each preset.
///
/// Kept in one place with the reasoning, because the difference between them is
/// the whole product decision and it should be arguable in a diff.
struct PresetValues {
    jpeg_quality: u32,
    jpeg_chroma_subsampling: caesium::parameters::ChromaSubsampling,
    jpeg_progressive: bool,
    /// Off in every preset. See `AdvancedOptions::lossless_optimize` for why
    /// this is not simply "turn trellis on for Strong".
    lossless_optimize: bool,
    png_quality: u32,
    png_optimization_level: u8,
    png_force_zopfli: bool,
    png_optimize: bool,
    webp_quality: u32,
    tiff_compression: caesium::parameters::TiffCompression,
}

fn preset_values(preset: CompressPreset) -> PresetValues {
    use caesium::parameters::{ChromaSubsampling as Cs, TiffCompression as Tc};

    match preset {
        CompressPreset::Balanced => PresetValues {
            // mozjpeg's quality scale is not libjpeg's. 80 is roughly what
            // libjpeg calls 90, and is the point where re-encoding twice is not
            // visible.
            jpeg_quality: 80,
            jpeg_chroma_subsampling: Cs::CS420,
            jpeg_progressive: true,
            lossless_optimize: false,
            // PNG's quality is a palette reduction, not a lossy codec knob, so
            // it only does anything on images with many colours. Left at the
            // maximum: shrinking a screenshot's palette is not what someone
            // pressing "compress" is asking for.
            png_quality: 100,
            png_optimization_level: 2,
            png_force_zopfli: false,
            png_optimize: true,
            webp_quality: 80,
            tiff_compression: Tc::Deflate,
        },
        CompressPreset::Strong => PresetValues {
            jpeg_quality: 65,
            jpeg_chroma_subsampling: Cs::CS420,
            jpeg_progressive: true,
            lossless_optimize: false,
            png_quality: 100,
            png_optimization_level: 4,
            png_force_zopfli: false,
            png_optimize: true,
            webp_quality: 65,
            tiff_compression: Tc::Deflate,
        },
        CompressPreset::Maximal => PresetValues {
            jpeg_quality: 50,
            jpeg_chroma_subsampling: Cs::CS420,
            jpeg_progressive: true,
            lossless_optimize: false,
            png_quality: 100,
            png_optimization_level: 6,
            // Deliberately *not* zopfli, despite the name. Measured on a real
            // 1.43 MB PNG: zopfli took 75 seconds and returned 1.28 MB, against
            // 1.29 MB in 1.4 seconds for Balanced - 0.7 percentage points for
            // fifty-five times the wait. A preset people click from a menu cannot
            // spend that. It is one checkbox in Advanced instead, for whoever
            // genuinely wants to wait overnight.
            png_force_zopfli: false,
            png_optimize: true,
            webp_quality: 50,
            tiff_compression: Tc::Deflate,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A photo-like test image built from a real PRNG.
    ///
    /// An earlier version used `(x * 7 + y * 13) % 251`, which is a repeating
    /// diagonal pattern. That compresses so well losslessly that lossy WebP came
    /// out *larger* than the source and the round-trip test failed for the wrong
    /// reason. Genuine noise resists both encoders, so any size reduction is real.
    fn noisy_png(path: &Path) {
        let mut img = image::RgbaImage::new(160, 120);
        // A fixed-seed LCG: reproducible, and uncorrelated between pixels.
        let mut state: u32 = 0x1234_5678;
        for pixel in img.pixels_mut() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = (state >> 24) as u8;
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let b = (state >> 24) as u8;
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let c = (state >> 24) as u8;
            *pixel = image::Rgba([a, b, c, 255]);
        }
        img.save(path).unwrap();
    }

    #[test]
    fn only_supported_formats_are_compressible() {
        assert!(is_compressible(Path::new("a.jpg")));
        assert!(is_compressible(Path::new("a.JPEG")));
        assert!(is_compressible(Path::new("a.tiff")));
        assert!(!is_compressible(Path::new("a.gif")));
        assert!(!is_compressible(Path::new("a.svg")));
        assert!(!is_compressible(Path::new("a.ico")));
        assert!(!is_compressible(Path::new("a.heic")));
        assert!(!is_compressible(Path::new("noextension")));
    }

    #[test]
    fn validation_rejects_a_text_file_named_like_an_image() {
        let dir = temp_dir("compress_validate");
        let fake = dir.join("not-really.jpg");
        std::fs::write(&fake, b"<!DOCTYPE html><html>404</html>").unwrap();
        assert!(validate_input(&fake).is_err(), "a text file must not be sent to a codec");

        let empty = dir.join("empty.png");
        std::fs::write(&empty, b"").unwrap();
        assert!(validate_input(&empty).is_err(), "an empty file must be rejected");

        let missing = dir.join("gone.png");
        assert!(validate_input(&missing).is_err());

        let real = dir.join("real.png");
        noisy_png(&real);
        assert!(validate_input(&real).is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compression_actually_shrinks_a_lossless_image() {
        let dir = temp_dir("compress_shrink");
        let src = dir.join("noisy.png");
        noisy_png(&src);
        let before = std::fs::metadata(&src).unwrap().len();

        let out = dir.join("out.png");
        let result = compress_file(&src, &out, &CompressParams::default()).unwrap();

        assert!(result.output_size < result.original_size);
        assert!(result.saved() > 0);
        assert!(result.ratio() > 0.0);
        assert!(!result.unchanged);
        assert!(!result.target_missed);
        assert!(out.exists());
        // The source is never touched.
        assert_eq!(std::fs::metadata(&src).unwrap().len(), before);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_cannot_be_improved_is_left_alone() {
        let dir = temp_dir("compress_unchanged");
        let src = dir.join("noisy.png");
        noisy_png(&src);
        let original = std::fs::metadata(&src).unwrap().len();

        let out = dir.join("out.png");
        // Downscaling noise to one pixel discards almost all of it, and the
        // re-encode can easily land at or above the original size. Either way
        // the original must survive.
        let params = CompressParams {
            max_dimension: Some(1),
            ..Default::default()
        };
        let result = compress_file(&src, &out, &params).unwrap();

        if result.unchanged {
            assert_eq!(result.output_size, original);
            assert_eq!(result.saved(), 0);
            assert!(!out.exists(), "nothing better was produced, so nothing is written");
        } else {
            assert!(result.output_size < original);
            assert!(out.exists());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn presets_are_ordered_by_how_much_they_give_up() {
        let balanced = build_parameters(&CompressParams {
            preset: CompressPreset::Balanced,
            ..Default::default()
        });
        let strong = build_parameters(&CompressParams {
            preset: CompressPreset::Strong,
            ..Default::default()
        });
        let maximal = build_parameters(&CompressParams {
            preset: CompressPreset::Maximal,
            ..Default::default()
        });

        assert!(balanced.jpeg.quality > strong.jpeg.quality);
        assert!(strong.jpeg.quality > maximal.jpeg.quality);
        assert!(balanced.webp.quality > maximal.webp.quality);
        // No preset may enable zopfli. Measured: 75 seconds on a 1.43 MB PNG for
        // 0.7 percentage points, against 1.4 seconds for Balanced's 10.2%.
        // A preset people pick from a menu cannot spend that.
        for preset in [
            CompressPreset::Balanced,
            CompressPreset::Strong,
            CompressPreset::Maximal,
        ] {
            let cs = build_parameters(&CompressParams {
                preset,
                ..Default::default()
            });
            assert!(
                !cs.png.force_zopfli,
                "{preset:?} must not enable zopfli"
            );
            // And every preset stays on the lossless PNG path, so nothing
            // silently quantises someone's artwork to 256 colours.
            assert!(cs.png.optimize, "{preset:?} must stay lossless for PNG");
        }
        // Every preset leaves the lossless JPEG path off. Turning it on makes
        // libcaesium skip `jpeg_set_quality` entirely, so a "stronger" preset
        // produced a *larger* file than the balanced one.
        for params in [
            CompressPreset::Balanced,
            CompressPreset::Strong,
            CompressPreset::Maximal,
        ] {
            let cs = build_parameters(&CompressParams {
                preset: params,
                ..Default::default()
            });
            assert!(
                !cs.jpeg.optimize,
                "{params:?} must not switch JPEG to the lossless path"
            );
        }
    }

    #[test]
    fn stronger_presets_actually_produce_smaller_files() {
        // The bug this pins: Strong and Maximal enabled `jpeg_optimize`, which
        // libcaesium treats as "recompress losslessly" and which skips the
        // quality setting. Both produced 751,503 bytes for every quality, worse
        // than Balanced's 684,987, so "Strong" was silently the wrong way up.
        let dir = temp_dir("compress_monotonic");
        let src = dir.join("photo.png");
        noisy_png(&src);

        let mut sizes = Vec::new();
        for preset in [
            CompressPreset::Balanced,
            CompressPreset::Strong,
            CompressPreset::Maximal,
        ] {
            let out = dir.join(format!("{preset:?}.jpg"));
            let result = compress_file(
                &src,
                &out,
                &CompressParams {
                    preset,
                    ..Default::default()
                },
            )
            .unwrap();
            sizes.push((preset, result.output_size));
        }

        for pair in sizes.windows(2) {
            assert!(
                pair[1].1 <= pair[0].1,
                "{:?} produced {} bytes, more than {:?} at {}",
                pair[1].0,
                pair[1].1,
                pair[0].0,
                pair[0].1
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn advanced_options_override_the_preset() {
        let params = CompressParams {
            preset: CompressPreset::Balanced,
            advanced: AdvancedOptions {
                quality: Some(42),
                progressive: Some(false),
                force_zopfli: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        let cs = build_parameters(&params);

        assert_eq!(cs.jpeg.quality, 42);
        assert_eq!(cs.png.quality, 42);
        assert_eq!(cs.webp.quality, 42);
        assert!(!cs.jpeg.progressive, "the override must win over the preset");
        assert!(cs.png.force_zopfli);

        // An unset option must leave the preset alone.
        let cs = build_parameters(&CompressParams::default());
        assert_eq!(cs.jpeg.quality, 80);
        assert!(cs.jpeg.progressive);
    }

    #[test]
    fn zero_dimension_means_keep_the_original_size() {
        let cs = build_parameters(&CompressParams::default());
        assert_eq!(cs.width, 0);
        assert_eq!(cs.height, 0);

        let cs = build_parameters(&CompressParams {
            max_dimension: Some(1600),
            ..Default::default()
        });
        assert_eq!(cs.width, 1600);
    }

    #[test]
    fn metadata_is_dropped_unless_asked_for() {
        let cs = build_parameters(&CompressParams::default());
        assert!(!cs.keep_metadata, "stripping is the default");

        let cs = build_parameters(&CompressParams {
            keep_metadata: true,
            ..Default::default()
        });
        assert!(cs.keep_metadata);

        // Rotation survives regardless, because losing it rotates the image.
        assert!(build_parameters(&CompressParams::default()).keep_rotation);
    }

    #[test]
    fn a_target_below_the_original_reports_whether_it_was_met() {
        let dir = temp_dir("compress_target");
        let src = dir.join("noisy.png");
        noisy_png(&src);
        let original = std::fs::metadata(&src).unwrap().len();

        let out = dir.join("out.png");
        // Ambitious but reachable: a fraction of the original.
        let result = compress_file(
            &src,
            &out,
            &CompressParams {
                target_size: Some(original / 2),
                ..Default::default()
            },
        )
        .unwrap();

        if !result.target_missed {
            assert!(
                result.output_size <= original / 2,
                "claimed the target was met at {} bytes, target was {}",
                result.output_size,
                original / 2
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_impossible_target_is_reported_as_missed_rather_than_faked() {
        let dir = temp_dir("compress_target_missed");
        let src = dir.join("noisy.png");
        noisy_png(&src);

        let out = dir.join("out.png");
        // A single byte cannot hold any image.
        let result = compress_file(
            &src,
            &out,
            &CompressParams {
                target_size: Some(1),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(result.target_missed, "an unmet target must be visible");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn output_never_overwrites_a_failed_run_with_a_partial_file() {
        let dir = temp_dir("compress_atomic");
        let src = dir.join("noisy.png");
        noisy_png(&src);
        let out = dir.join("out.png");

        // Leave rubbish at the staging path first.
        let stage = stage_path(&out);
        let mut junk = std::fs::File::create(&stage).unwrap();
        junk.write_all(b"stale rubbish").unwrap();
        drop(junk);

        let result = compress_file(&src, &out, &CompressParams::default()).unwrap();
        assert!(!result.unchanged);

        // The stale file must be gone, not promoted.
        let written = std::fs::read(&out).unwrap();
        assert_ne!(written, b"stale rubbish");
        assert!(!stage.exists(), "the staging file should not survive");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_source_is_never_modified() {
        let dir = temp_dir("compress_immutable");
        let src = dir.join("noisy.png");
        noisy_png(&src);
        let before = std::fs::read(&src).unwrap();

        let out = dir.join("out.png");
        compress_file(&src, &out, &CompressParams::default()).unwrap();

        assert_eq!(std::fs::read(&src).unwrap(), before);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_image_fails_without_producing_anything() {
        // Release builds use `panic = "abort"`, so a codec that panics on bad
        // input takes the whole app with it. Every malformed case has to come
        // back as an ordinary error instead, with no partial file left behind.
        let dir = temp_dir("compress_corrupt");

        let cases: &[(&str, &[u8])] = &[
            // Valid JPEG header, nothing else.
            ("truncated.jpg", &[0xFF, 0xD8, 0xFF]),
            // Valid PNG signature, no IHDR.
            ("truncated.png", &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            // Correct signature, corrupt body.
            ("garbage.png", b"\x89PNG\r\n\x1a\nthis is not an image at all"),
            // A JPEG header followed by random noise.
            ("noisy.jpg", &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', 0x01, 0x02]),
            // RIFF/WEBP with a bogus payload.
            ("fake.webp", b"RIFF\x10\x00\x00\x00WEBPgarbage"),
            // TIFF byte-order mark, nothing else.
            ("stub.tiff", &[0x49, 0x49, 0x2A, 0x00]),
        ];

        for (name, bytes) in cases {
            let src = dir.join(name);
            std::fs::write(&src, bytes).unwrap();
            let out = dir.join(format!("{name}.out"));

            let outcome = compress_file(&src, &out, &CompressParams::default());

            match outcome {
                Ok(result) => {
                    // If it claims success it must be a real file, never the
                    // junk we fed it.
                    assert!(
                        out.exists() && std::fs::metadata(&out).unwrap().len() > 0,
                        "{name}: reported success but wrote nothing"
                    );
                    let _ = result;
                }
                Err(_) => {
                    assert!(!out.exists(), "{name}: failed but still wrote a file");
                }
            }

            // No staging leftovers in either case.
            let stage = stage_path(&out);
            assert!(!stage.exists(), "{name}: left a staging file behind");

            let _ = std::fs::remove_file(&src);
            let _ = std::fs::remove_file(&out);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_supported_format_round_trips() {
        // Proves the whitelist is not aspirational: every extension the manifest
        // advertises has to actually survive a real encode.
        let dir = temp_dir("compress_formats");

        let source = dir.join("source.png");
        noisy_png(&source);
        let img = image::open(&source).unwrap();

        for ext in ["jpg", "png", "webp", "tiff"] {
            let src = dir.join(format!("in.{ext}"));
            img.save(&src).unwrap();

            let out = dir.join(format!("out.{ext}"));
            let result = compress_file(&src, &out, &CompressParams::default())
                .unwrap_or_else(|e| panic!("{ext} failed to compress: {e}"));

            assert!(
                result.output_size <= result.original_size,
                "{ext} grew from {} to {}",
                result.original_size,
                result.output_size
            );

            if result.unchanged {
                // Legitimate: nothing better was available, so nothing was
                // written. The point of the test is that the format works.
                assert!(!out.exists(), "{ext} claimed unchanged but wrote a file");
                continue;
            }

            assert!(out.exists(), "{ext} produced no file");
            assert!(result.output_size > 0, "{ext} produced an empty file");

            // And the result must still be a decodable image of the same shape.
            let reopened = image::open(&out)
                .unwrap_or_else(|e| panic!("{ext} output did not reopen: {e}"));
            assert_eq!(
                reopened.width(),
                img.width(),
                "{ext} changed the width"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}