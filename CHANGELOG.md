# Changelog

## v0.1.0-pre-alpha.2 — 2026-10-10

### Added

- **Compress Image.** Shrinks an image and keeps the same format. Three presets, plus optional resize, a target file size, and per-format settings in Advanced. Supports JPG, JPEG, PNG, WebP and TIFF.
- Lossy WebP is now available through Compress.

### Fixed

- Tool windows now open on the monitor your cursor is on, instead of always the primary one.
- A window larger than the screen no longer opens partly off the left edge.

### Known issues

- Compress has no progress bar or cancel button.
- Compress to a target size can be unreachable on PNG. The app says so when it happens.
- Convert to WebP still writes lossless WebP, so converting a photo can make it larger.

## v0.1.0-pre-alpha.1 — 2026-09-30

### Added

- **Recolor Image.** Replace colours by rule with a tolerance slider, or flatten an image to one colour. SVG is edited as markup, so gradients and structure survive.
- First-run setup window.
- FFmpeg is now detected, downloaded and installed for you.

### Changed

- Folders are picked with the modern Windows dialog.
- Settings no longer carries migration code, as Lime had not shipped yet.

### Fixed

- The wheel showed no tools at all for a JPEG.
- Trim no longer offers itself for SVG, where it always failed.
- Petal spacing is correct on wheels with few tools.