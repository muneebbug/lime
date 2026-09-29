# Wheel

Wheel is a Windows 10/11 desktop background application that displays a radial drag-and-drop toolkit whenever you hold **Shift** while dragging one or more files. Dropping files onto a wedge either triggers a background format conversion or opens a specialized tool window.

Wheel is completely local-first: all image processing, PDF operations, model inference, and media conversions run locally on your machine with zero network telemetry.

---

## Capabilities

### Instant Conversions
- **Images:** PNG, JPG, WEBP, AVIF, TIFF, BMP, and ICO (with multi-resolution mipmap generation: 16, 24, 32, 48, 64, 128, 256 px).
- **Documents:** Multi-image to PDF assembly, raster extraction from existing PDFs.
- **Media (FFmpeg):** MP4, WebM, MOV, MKV, palette-optimized 15fps GIF generation, audio extraction to MP3, WAV, FLAC, and M4A.

### Tool Windows
- **Crop:** Freeform or aspect ratio presets (1:1, 16:9, 9:16, 4:3, 3:4) with synchronized width/height pixel inputs and DPI-crisp SVG handles.
- **Compress:** Visual balance presets ("Balanced" vs. "Strong") and target file size binary search.
- **Metadata:** EXIF, Camera, Lens, and GPS inspection with one-click copy, Google Maps coordinate links, and selective or complete metadata stripping.
- **Add Background:** Canvas padding, corner radius masking, soft drop shadow blur, gradient/solid color swatches, and aspect ratio padding (Auto, 1:1, 16:9, 4:5).
- **Edit:** Real-time CSS filter adjustments (brightness, contrast, saturation, warmth) paired with 90° rotation, flips, dimension resizing with aspect lock, and Lanczos3 resampling.
- **Remove Background:** High-accuracy RMBG-1.4 neural model with on-demand download manager, fallback saliency segmenter, before/after split slider, and edge feathering.
- **Redact:** Irreversible in-memory pixel destruction with black bar, 16×16 average block pixelation, and heavy Gaussian blur. Strips EXIF metadata automatically on export.
- **Annotate:** High-resolution vector overlay composited directly onto source image pixels. Pen, semi-transparent highlighter, arrowheads, rectangles, text, auto-incrementing numbered step markers (1, 2, 3...), and undo/redo stacks.

### Productivity Surfaces
- **Radial Wheel Overlay:** Pre-created transparent Win32 window with polar geometry and per-monitor DPI boundary clamping (<50ms trigger-to-frame latency).
- **Command Palette (`Ctrl+Alt+Space`):** Spotlight-style floating search across tools, conversions, preset chains, and recent conversion history.
- **Action Presets & Chains:** Multi-step automated recipes (e.g. *Clean Web Asset*: strip metadata → convert to WebP).
- **Windows Explorer Context Menu:** One-click integration under `HKCU\Software\Classes\*\shell\Wheel` without requiring administrator privileges.
- **Settings Window:** Tabbed preferences interface for configuring trigger keys, drag thresholds, wheel size, wedge count, and output naming policies.
- **SQLite History & Recycle Bin:** Persistent job history at `%LOCALAPPDATA%\Wheel\history.db` with non-destructive Windows Recycle Bin integration.

---

## Architecture

The project is structured as a Cargo workspace with a TypeScript/React desktop client powered by Tauri 2:

```
wheel/
├── apps/
│   └── desktop/            # Tauri 2 app (React, Tailwind CSS v4, Motion, Lucide)
│       └── src-tauri/      # Tauri backend, state management, Win32 hooks listener
├── crates/
│   ├── wheel-core/         # Actions registry, job queue, versioned settings, SQLite history
│   ├── wheel-win/          # Low-level Win32 hooks (WH_MOUSE_LL, WH_KEYBOARD_LL), OLE IDropTarget, DPI clamping, shell
│   └── wheel-engines/      # Image conversions, PDF, metadata, redaction, annotation, background removal, FFmpeg
└── docs/
    ├── DECISIONS.md        # Architectural decision records (D001–D033)
    └── TESTING.md          # Multi-monitor DPI, OLE drag-and-drop, and engine test matrix
```

### Safety & Invariants
1. **OLE Drop Safety:** Drop target always returns `DROPEFFECT_COPY` (never `DROPEFFECT_MOVE`) so Windows Explorer never deletes or moves source files upon drop.
2. **Elevation-Free Shell Integration:** Registry keys are registered under `HKEY_CURRENT_USER` so the context menu can be toggled without UAC elevation.
3. **Irreversible Redactions:** Redacted regions destroy underlying pixel values in memory before saving; vector masks or reversible CSS overlays are strictly banned.
4. **Zero Shell Concatenation:** FFmpeg executions use safe `std::process::Command` argument vectors without shell string interpolation.

---

## Prerequisites

- **Windows 10 / 11** (x86_64)
- **Rust:** 1.77+ with `x86_64-pc-windows-msvc` target
- **Node.js:** 18+ and `pnpm` (or `npm`)
- **FFmpeg (Optional):** Automatically detected in system `PATH` or at `%LOCALAPPDATA%\Wheel\bin\ffmpeg.exe`.

---

## Development & Building

### 1. Install Dependencies
```powershell
pnpm install
```

### 2. Run All Tests
```powershell
cargo test --workspace
```

### 3. Run Development Server
```powershell
pnpm --filter desktop tauri dev
```

### 4. Build Production Bundle
```powershell
pnpm --filter desktop tauri build
```
The installer will be generated in `apps/desktop/src-tauri/target/release/bundle/nsis/`.

---

## Configuration

Wheel stores configuration and caches in `%LOCALAPPDATA%\Wheel\`:
- `history.db`: SQLite database storing job execution records.
- `models/`: Neural segmentation models (e.g. `rmbg-1.4.onnx`).
- `bin/`: Optional sidecar tools (e.g. `ffmpeg.exe`).

Application settings can be adjusted in the UI via the tray icon menu ("Open Settings") or via the Command Palette (`Ctrl+Alt+Space`).
