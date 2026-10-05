<div align="center">
  <img src="apps/desktop/src/assets/logo.svg" alt="Lime" width="180" />
  <h1>Lime</h1>
  <p>A Windows desktop app for converting images, video and audio, driven by a wheel that appears under your cursor while you drag files.</p>
</div>

<div align="center">

[![CI](https://github.com/muneebbug/lime/actions/workflows/ci.yml/badge.svg)](https://github.com/muneebbug/lime/actions/workflows/ci.yml)
[![Release](https://github.com/muneebbug/lime/actions/workflows/release.yml/badge.svg)](https://github.com/muneebbug/lime/actions/workflows/release.yml)
[![Version](https://img.shields.io/github/package-json/v/muneebbug/lime)](https://github.com/muneebbug/lime/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2B%2064--bit-0078d4)](https://github.com/muneebbug/lime/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-cbe71f.svg)](LICENSE)

[Download](https://github.com/muneebbug/lime/releases) · [Report an issue](https://github.com/muneebbug/lime/issues)

</div>

---

> [!WARNING]
> **Lime is a pre-alpha build.** Expect bugs, missing features and an interface that changes without warning. If something behaves wrongly, check the [open issues](https://github.com/muneebbug/lime/issues) first in case it is already known, and otherwise [open a report](https://github.com/muneebbug/lime/issues/new) — reports with the file that triggered it are the most useful.

## How it works

1. Select one or more files in File Explorer.
2. Start dragging them and hold **Shift**.
3. A wheel appears around your cursor, showing only the actions that make sense for what you picked.
4. Move onto the action you want and release. The result is written next to the original file.

Nothing is uploaded. Every conversion happens on your machine.

## What it converts

**Images** — PNG, JPG, WebP, TIFF, AVIF, BMP, GIF, ICO, PDF. SVG and most raster formats are accepted as input.

**Video** — MP4, WebM, MOV, MKV, AVI, and animated GIF.

**Audio from video** — MP3, WAV, M4A, FLAC.

**Audio** — MP3, WAV, M4A, FLAC, AAC, OGG, Opus, WMA.

### Detail worth knowing

- **PNG, TIFF, WebP and AVIF are written losslessly**, so a round trip through them does not change a single pixel. JPEG has no lossless mode, so it is written at maximum quality instead.
- **Metadata is kept, per media type.** Images carry EXIF and colour profiles into JPEG, PNG and WebP; audio carries tags and cover art; video carries tags. Each can be switched off in Settings.
- **Transparent pixels survive.** AVIF in particular round-trips exactly, so trimming a transparent PNG to AVIF and back does not damage the edges.

## Trim

The one built-in tool. Drop an image on the **TRIM** petal and it crops away fully transparent borders, keeping the smallest rectangle that contains every visible pixel.


## Requirements

- Windows 10 or 11, 64-bit
- [FFmpeg](https://ffmpeg.org/) on your `PATH`

FFmpeg is not bundled. Lime looks for `ffmpeg.exe` in `%LOCALAPPDATA%\Lime\bin\`, then next to `Lime.exe`, then on your system `PATH`. Image conversions work without it; video, audio and PDF output need it.

---

## Development

Built with [Tauri 2](https://tauri.app/) — a Rust core with a React and TypeScript frontend.

```powershell
pnpm install
```

Run it in development mode:

```powershell
pnpm dev
```

Run the Rust test suite:

```powershell
cargo test --workspace
```

Build the Windows installer:

```powershell
pnpm build
```

The installer lands in `apps/desktop/src-tauri/target/release/bundle/nsis/`. It installs per-user, so no administrator rights are needed.

### Layout

```
apps/desktop/          Tauri app — React frontend and Rust commands
  src/                 React UI, including the wheel and settings window
  src-tauri/src/       Rust: window management, tray, settings persistence
crates/wheel-core/     Settings, action registry, job queue, history database
crates/wheel-engines/  Conversion engines: image, media, PDF, trim
crates/wheel-win/      Windows shell integration
```

The two crates under `crates/` have no Tauri dependency, so the conversion logic can be tested and reasoned about on its own. `cargo test --workspace` runs the whole suite.

### Where your data lives

Everything is written under `%LOCALAPPDATA%\Lime\`:

- `settings.json` — preferences
- `history.db` — SQLite database of past conversions

---

## Not done yet

- Hardware-accelerated conversion for NVIDIA, AMD and Intel GPUs
- Converting PDF pages back into images
- Smaller video-to-GIF output
- Carrying EXIF through TIFF conversions

## Contributing

Open an issue or a pull request. Bug reports with the file that caused it are the most useful kind.

## License

MIT. See [LICENSE](LICENSE).