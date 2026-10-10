# Changelog

All notable changes to Lime. Versions are pre-alpha and the interface is still
moving; see the warning at the top of the [README](README.md).

## v0.1.0-pre-alpha.2

*Released 2026-10-10*

### Added — Compress Image

A third tool alongside Trim and Recolor. Drop an image on the **COMPRESS** petal
and it re-encodes it smaller, in place, without changing what it is — the output
keeps the input's format, because compressing is not converting.

- **Three presets.** Balanced (visually lossless in most cases, the default),
  Strong, and Maximal.
- **Basic settings**: preset, longest-edge resize, compress-to-target-file-size,
  and whether to keep EXIF. Image orientation is always kept, so nothing comes
  out rotated.
- **Advanced settings**, per format: chroma subsampling, progressive JPEG,
  lossless re-compression, PNG mode, zopfli, lossless WebP, TIFF algorithm and
  deflate effort, and ICC profile retention. Every control shows what the preset
  chose and can be reset individually.
- **Lossy PNG.** The only way to get a large reduction from a photo-like PNG is
  to quantise the palette, which is lossy, so it is an explicit choice rather
  than a default.
- **Target file size** searches for the smallest result under your limit, and
  says so plainly when the limit cannot be reached.
- A **size estimate** before you start for JPEG, WebP and TIFF. There is none
  for lossless PNG, because there is no honest curve for it.

Compression is backed by [libcaesium](https://crates.io/crates/libcaesium),
which wraps mozjpeg, oxipng and libwebp. That also makes lossy WebP available
for the first time — the image crate can only write lossless WebP.

Accepted: **JPG, JPEG, PNG, WebP, TIFF, TIF**. Deliberately not GIF (its
compression path is partial, and re-encoding an animation risks flattening it to
one frame), SVG (shrinking one would mean rasterising it), ICO (already packed),
AVIF (already compressed) or HEIC (cannot be decoded).

### Fixed

- **Tool windows now open on the monitor you are using.** They were centred on
  the primary display regardless of where the tool was activated, so on a
  multi-monitor setup a window could appear somewhere you were not looking.
  Windows are now centred on the cursor, clamped into that monitor's work area,
  and an already-open tool follows you rather than staying where it was.
- A window larger than its work area no longer opens with a **negative
  coordinate**, which put part of it off the left of the screen. The clamping
  bounds could invert when the window did not fit.

### Known limitations

- Compress has no progress reporting or cancellation. Slow runs show a spinner
  and nothing else.
- "Compress to target file size" ignores the quality slider — the search picks
  its own — and on PNG the target is often unreachable, which the window reports.
- **Convert to WebP still writes lossless WebP**, so converting a photo can make
  it larger. Compress does this correctly; the convert target has not been
  re-pointed at it yet.

## v0.1.0-pre-alpha.1

*Released 2026-09-30*

First published build.

### Added

- **Recolor Image.** Replace any colour by rule, or flatten to one. Matching is
  in OKLab, so a rule catches the shades the eye calls the same colour. Rules can
  repaint flat or carry each pixel's lightness through. SVG sources are edited as
  markup, so gradients, structure and hand-written ids survive.
- **First-run setup** window.
- **FFmpeg is detected, downloaded and installed** for you, with progress, size
  and speed; downloads use parallel ranges from the fastest mirror.
- **Modern folder picker** for Settings, replacing the old tree-view dialog.

### Changed

- Colours moved into CSS tokens with a shared window frame, so every window has
  the same chrome.
- Settings dropped its migration code. Lime had not shipped, so there was nothing
  to migrate; "Ask Each Time" for output folders went with it.

### Fixed

- The wheel filtered every tool out for a JPEG, because Recolor was declared as
  accepting only formats with an alpha channel. The Tools page came up empty and
  nothing could be hovered.
- Trim no longer offers itself for SVG. It rasterised the file before cropping
  and could not write the result back as a vector, so it always failed on save.
- Petal corner geometry adapted to low petal counts, which had been leaving
  crescent gaps on a two-tool wheel.