# Wheel — Comprehensive Verification & Test Matrix

This document defines the automated test suites, manual verification matrix, performance targets, and security invariants across Windows 10/11 environments.

---

## 1. Automated Test Suites

### Rust Workspace Test Suite
Run all unit and integration tests across the 4 workspace crates:
```powershell
cargo test --workspace
```

| Crate | Test Coverage | Status |
|-------|---------------|--------|
| `wheel-core` | Output path resolution (`resolve_output_path`), action registry filtering, versioned settings schema serialization & migration, SQLite history CRUD (`HistoryDb`) | 9 / 9 Passing |
| `wheel-engines` | Image conversions (PNG, JPG, WEBP, BMP, TIFF, GIF, ICO), Lopdf image assembly & extraction, EXIF metadata extraction & complete stripping, image transforms (90° rotations, flips, Lanczos3 resize), crop bounds, binary-search compression, RMBG-1.4 background removal, irreversible pixel burn redactions, full-resolution annotation overlay blending, FFmpeg process detection | 12 / 12 Passing |
| `wheel-win` | Explorer context menu registry query & lifecycle (`is_context_menu_registered`) | 1 / 1 Passing |
| `desktop` | Tauri command generation & AppState registration | Clean Build |

### Frontend Build Verification
Verify TypeScript type-checking and production bundle asset compilation:
```powershell
pnpm --filter desktop build
```
- Bundler: Vite v8 + React 19 + TypeScript
- Zero TS errors, zero unassigned imports.

---

## 2. Manual Verification Matrix

### Drag & Drop Sources (OLE Data Transfer)

| Source | Trigger (Shift+Drag) | Overlay Display | File Resolution | Source Preservation |
|--------|----------------------|-----------------|-----------------|---------------------|
| Windows 11 File Explorer | ✅ Triggered | ✅ Centered | ✅ `CF_HDROP` path parsed | ✅ `DROPEFFECT_COPY` (untouched) |
| Desktop Surface Icons | ✅ Triggered | ✅ Centered | ✅ Shell namespace resolved | ✅ Untouched |
| Edge / Chrome Download Shelf | ✅ Triggered | ✅ Centered | ✅ Downloaded file parsed | ✅ Untouched |
| Outlook / Teams Desktop App | ✅ Triggered | ✅ Centered | ✅ Temp attachment path parsed | ✅ Untouched |
| Directory Selection | ✅ Triggered | ✅ Centered | ✅ Folder context recognized | ✅ Untouched |

### Multi-Monitor & DPI Scenarios

| Display Setup | Expected Behavior | Verification |
|---------------|-------------------|--------------|
| 100% DPI (96 DPI) Standard Display | 320px radial overlay centered on cursor | Passed |
| 125% DPI (120 DPI) Laptop Display | Overlay scales crisp SVG vectors without blur; clamped to work area | Passed |
| 150% DPI (144 DPI) 4K Display | DPI-aware coordinate translation; mouse hit-testing matches visuals | Passed |
| Mixed DPI (e.g. 150% Primary + 100% Secondary) | Clamped to the active monitor work area; handles negative desktop coordinates | Passed |
| Screen Edge Clamping | Cursor at screen edge clamps overlay to visible screen boundaries (<50ms) | Passed |

### Conversion Engine Matrix

| From Format | To Format | Quality Target | Speed | Verification Notes |
|-------------|-----------|----------------|-------|-------------------|
| PNG / JPG | WEBP | Lossy quality 85, high fidelity | <150ms | Strips EXIF by default |
| PNG / WEBP | JPG | Pure RGB8 buffer (alpha flattened) | <120ms | No alpha channel rejection |
| PNG / JPG | AVIF | Hardware-accelerated AV1 (-cpu-used 8) | ~650ms | High-efficiency next-gen compression |
| PNG / JPG | ICO | Auto-resized 256x256 multi-resolution icon | <100ms | Direct Windows icon format |
| Any Image | PDF | Multi-image Lopdf assembly | <250ms | Scaled to standard points |
| PDF Document | PNG / JPG | Raster extraction of embedded bitmaps | <300ms | Preserves original raster quality |
| MP4 / MOV Video | GIF | 15fps, Lanczos downscale, palettegen/paletteuse | ~2-4s | Vibrant palette, no color banding |
| MP4 / WebM Video | MP3 / WAV | Audio track extraction via safe arguments | ~1-2s | 192k audio or 16-bit PCM |

### Tool Windows Verification

| Tool | Action | Acceptance Criteria |
|------|--------|---------------------|
| **Crop** | Drag 8 SVG handles or pick aspect ratio preset (1:1, 16:9, etc.) | Real-time pixel width/height inputs sync; output is cropped cleanly |
| **Compress** | Choose "Balanced" or "Strong" preset or target file size | Binary-search converges on target size; visual preview displays estimated savings |
| **Metadata** | View EXIF, Camera, Lens, GPS; click Strip GPS or Strip All | kamadak-exif cleans tags; GPS coordinates link cleanly to Google Maps |
| **Add BG** | Adjust padding, blur, rounded corners, gradient/solid swatches | Drop shadow and rounded corner mask rendered at 60fps; exported at native resolution |
| **Edit** | Rotate 90°, flip H/V, adjust brightness/contrast/saturation | Fast CSS filter preview at 60fps; backend applies Lanczos3 resampling upon export |
| **Remove BG**| RMBG-1.4 neural model with fallback segmenter | Split slider compares before/after; edge feathering smooths cutouts |
| **Redact** | Black bar, 16×16 block pixelate, heavy Gaussian blur | Irreversible pixel memory destruction; orange alert badge confirms metadata stripped |
| **Annotate** | Pen, highlighter, arrow, rect, text, 1-2-3 step markers | Vector overlay rendered at full natural resolution; composited losslessly onto source image |

### Productivity Surfaces

| Feature | Trigger | Expected Behavior |
|---------|---------|-------------------|
| **Command Palette** | `Ctrl+Alt+Space` or Settings launcher | Floating `640x480` borderless window; fuzzy search across tools, converts, presets, and history |
| **Action Presets** | Preset wedge or Palette | Executes multi-step chain (e.g. *Clean Web Asset*); cleans intermediate temporary files |
| **Explorer Context Menu**| Right-click any file in Windows Explorer | "Open with Wheel" appears in menu without requiring administrator elevation |
| **Settings Window** | Tray menu "Open Settings" or Palette | Tabbed Raycast-style interface; modifies schema live with instant persistence |

---

## 3. Performance & Latency Targets

- **Trigger-to-Frame Latency:** < 50ms from mouse drag threshold detection to initial overlay render.
- **Radial Wedge Hover:** 0 memory allocations in JavaScript event loop; pure polar angle hit-testing (`atan2`).
- **OLE Drop Acceptance:** Returns `DROPEFFECT_COPY` within < 20ms of `IDropTarget::Drop`.
- **Memory Footprint:** < 45 MB background resident RAM when idle in system tray.

---

## 4. Security Invariants

1. **Local-First / Zero Cloud:** No telemetry, analytics, or remote API endpoints. Everything executes on the local CPU/GPU.
2. **Reversible File Operations:** The source file is never overwritten unless explicitly requested. Moving to Recycle Bin uses the native Windows Shell `IFileOperation` with complete undoability.
3. **Command Execution Safety:** All sidecar and FFmpeg invocations use explicit argument vectors (`std::process::Command::arg`). No shell string concatenation is permitted.
4. **Permanent Redactions:** Pixels in redacted regions are irreversibly destroyed in memory before file serialization.
