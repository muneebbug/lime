# Wheel — Design Decisions

This document records every significant technical decision made during development,
its rationale, and the alternatives considered. Listed chronologically.

---

## M0 — Spike

### D001: Tauri 2.x (not 1.x)
**Decision:** Use Tauri 2.x (current stable at scaffold time: 2.12.0).
**Rationale:** Tauri 2.x has a capabilities/permissions system, better plugin ecosystem,
and the new tray API needed for a tray-only app. The 1.x branch is EOL for new projects.
**Alternative:** Tauri 1.x — rejected because it lacks per-window capability scoping and
the new plugin format.

### D002: Pre-created hidden overlay window
**Decision:** Create the overlay window at app startup and hide it; show/hide on demand.
**Rationale:** WebView2 window creation takes 100-300ms on first creation. Pre-creating
ensures the <50ms trigger-to-frame requirement is met. The window is hidden at `visible: false`
and repositioned before showing.
**Alternative:** Create the window on demand — rejected; too slow for the 50ms bar.

### D003: Custom IDropTarget (not Tauri's built-in drag-drop)
**Decision:** Implement a Win32 `IDropTarget` on the overlay HWND, disable Tauri's
`dragDropEnabled` for that window.
**Rationale:** Tauri's built-in drag-drop only fires after the user releases. We need
`DragEnter` to confirm the drag contains files (within 250ms), show context-aware wedges
during `DragOver`, and return `DROPEFFECT_COPY` to prevent Explorer from moving/deleting.
**Alternative:** Tauri built-in — rejected; cannot return a custom DROPEFFECT or get
early DragEnter events.

### D004: Always return DROPEFFECT_COPY
**Decision:** The IDropTarget always returns `DROPEFFECT_COPY`, never `DROPEFFECT_MOVE`.
**Rationale:** When Shift is held during an Explorer drag, the default effect is Move.
Returning COPY prevents the source file from being deleted if the user releases inside
our wheel. This is critical for user trust.

### D005: WH_MOUSE_LL + WH_KEYBOARD_LL on a dedicated thread
**Decision:** Install global low-level hooks on a dedicated Win32 message-pump thread.
Hook callbacks push to a lock-free unbounded channel; Tokio reads and processes.
**Rationale:** Hook callbacks must return in <100ms or Windows silently removes them.
Any non-trivial processing (window lookup, async await) would violate this. The lock-free
channel allows the callback to be trivially fast.
**Alternative:** Windows UI Automation or SetWinEventHook — rejected; cannot detect
movement threshold reliably.

### D006: Monorepo with three crates
**Decision:** `wheel-core` (no OS deps), `wheel-win` (Win32 only), `wheel-engines` (processing).
**Rationale:** `wheel-core` must remain unit-testable without a Windows environment.
`wheel-win` isolates all `unsafe` Win32 code. `wheel-engines` can be tested with fixture files.
**Alternative:** Single crate — rejected; makes testing impossible on non-Windows CI.

### D007: Tailwind CSS v4 (Vite plugin)
**Decision:** Use Tailwind v4 with the `@tailwindcss/vite` plugin (not PostCSS).
**Rationale:** Tailwind v4 is the current stable release (4.3.3). The Vite plugin
is the recommended setup per the official docs. No `tailwind.config.js` required.
**Alternative:** PostCSS setup — deprecated for v4; not recommended.

### D008: `motion` (not `framer-motion`) for animations
**Decision:** Import from `motion` package (version 13.4.4).
**Rationale:** Framer Motion was renamed to `motion` when it became framework-agnostic.
The current package name is `motion`; `framer-motion` is a compatibility alias that
may not stay current.
**Reference:** https://motion.dev

### D009: SVG for radial wheel geometry
**Decision:** Render wedges as SVG `<path>` elements computed from polar math, not Canvas.
**Rationale:** SVG is resolution-independent, integrates with CSS/Motion animations,
and does not require a Canvas 2D context. The wheel is small (320px) so SVG overhead is minimal.
**Alternative:** Canvas (react-konva) — reserved for tool windows (crop handles, annotate).

### D010: Elevated-app drag handling
**Decision:** Document known limitation: OLE drops from elevated processes to a non-elevated
Wheel instance may be blocked by UIPI. Detect and show a one-time explanation.
**Rationale:** There's no clean workaround without running Wheel elevated (which would
require UAC). The low-level hooks still detect the drag trigger regardless of elevation;
only the OLE drop is blocked.

### D011: `window-vibrancy` version 0.6
**Decision:** Use window-vibrancy 0.6 for Mica/Acrylic effects.
**Rationale:** Version 0.6 is the latest stable at time of scaffold (2026-09-28).
Checked via `cargo info window-vibrancy`.

### D012: Profiles at workspace root
**Decision:** Placed `[profile.release]` at workspace root `Cargo.toml` and removed from `apps/desktop/src-tauri/Cargo.toml`.
**Rationale:** Cargo ignores subpackage profiles in workspaces. Root configuration eliminates warnings and applies LTO, panic=abort, strip, and opt-level=3 across all crates.

### D013: `windows` crate 0.61 COM bindings and STGMEDIUM representation
**Decision:** Implement OLE `IDropTarget_Impl` using `windows::core::Ref<'_, IDataObject>`, `windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS`, and access `medium.u.hGlobal` directly.
**Rationale:** Windows crate 0.61 restructured COM types. STGMEDIUM unions use `.u` instead of `.Anonymous`, and `GetData` returns `Result<STGMEDIUM>`. Safe resource management is preserved by calling `ReleaseStgMedium(&mut medium as *mut _)`.

### D014: `ScreenToClient` translation for OLE DragOver and Drop hit-testing
**Decision:** Convert OLE drop screen coordinates (`POINTL`) to window client coordinates via Win32 `ScreenToClient(hwnd, &mut pt)` before emitting events to the frontend.
**Rationale:** In Windows OLE drag operations, standard DOM mousemove events are blocked by the OS mouse capture. Emitting accurate window client coordinates (0..400) allows the frontend `hitTestWedge` to highlight wedges in real time during drag over and select the correct wedge on drop.

### D015: Normalized extension filtering in AcceptedInput
**Decision:** Strip leading dots and perform case-insensitive comparison (`clean = ext.trim_start_matches('.').to_lowercase()`) in `AcceptedInput::accepts_extension`.
**Rationale:** Prevents context-filtering false negatives when files with uppercase extensions (e.g. `.PNG`, `.JPG`) or raw extensions with dots are passed from shell/Explorer.

---

## M2 — Conversions & Engines

### D016: `lopdf` for multi-image to PDF generation and raster image extraction
**Decision:** Use `lopdf` (version 0.45, feature `embed_image`) to assemble single and multi-page PDFs from arbitrary image inputs and to extract embedded raster images from PDFs.
**Rationale:** `lopdf` provides pure Rust PDF document creation, direct XObject stream generation, and lossless JPEG embedding via DCTDecode streams without relying on external native binaries or GPL tools.

### D017: Bundled SQLite via `rusqlite` for persistent job history
**Decision:** Store job execution history in `%LOCALAPPDATA%\Wheel\history.db` using `rusqlite` with the `bundled` feature.
**Rationale:** The `bundled` feature builds an isolated SQLite engine directly into the binary with zero runtime OS dependencies or DLL requirements. WAL journal mode ensures high concurrency and fast writes without UI lag.

### D018: Native Windows Recycle Bin integration via `trash`
**Decision:** When `output.recycle_source` is enabled, source files are sent to the Windows Recycle Bin using the `trash` crate (version 5.2).
**Rationale:** Invoking the Windows Shell `IFileOperation` API via `trash` ensures all file removals are completely undoable by the user via Windows Explorer (`Ctrl+Z`), preserving user trust.

### D019: WinRT `Windows.Data.Pdf` in `wheel-win`
**Decision:** Configure `Data_Pdf`, `Storage`, and `Storage_Streams` features in `wheel-win` for native Windows 10/11 PDF Direct2D rendering.
**Rationale:** Windows 10/11 includes a high-performance vector PDF rendering engine in WinRT. Exposing it in `wheel-win` avoids shipping bloated external C++ rendering libraries like Poppler.

---

## M3 — Tool Windows Framework + Crop + Compress + Metadata

### D020: Shared frameless Win11 window chrome (`ToolWindowLayout`)
**Decision:** Standardize all tool windows on `ToolWindowLayout` with rounded corners (Win11 style), warm radial gradient tint, circular close (X) button on left, centered draggable header with `data-tauri-drag-region`, and dual-action footer (Reset on left, primary action on right).
**Rationale:** Provides visual consistency across all tool windows matching the macOS/Win11 hybrid aesthetic specified in Section 7.2. Escape and Enter hotkeys operate uniformly across tools.

### D021: SVG-based interactive crop overlay with pixel synchronization
**Decision:** Implement the crop window using an interactive SVG crop mask and 8 draggable corner/edge handles that maintain two-way synchronization with explicit pixel inputs (W, H, X, Y) and aspect ratio presets (Free, 1:1, 16:9, 9:16, 4:3, 3:4).
**Rationale:** SVG provides high performance without adding heavy canvas dependencies, and allows standard DOM event handling and crisp scaling across arbitrary display DPIs.

### D022: Binary-search quality optimization for target-size compression
**Decision:** Implement image compression in `wheel-engines` supporting both preset modes (Balanced: 78% quality; Strong: 60% quality with downscaling) and a binary search over quality/scale when a target file size is specified.
**Rationale:** Users frequently need to hit strict upload size constraints (e.g. email or Discord attachments under 2MB). Binary search converges in ~5-6 iterations without arbitrary guesswork.

### D023: `kamadak-exif` for local EXIF/GPS parsing and clean pixel stripping
**Decision:** Use `kamadak-exif` (version 0.6) for structured metadata inspection and provide both "Strip GPS Only" and "Strip All Metadata" options. Full metadata stripping re-encodes pure pixel data, guaranteeing zero leaked metadata headers.
**Rationale:** Privacy-sensitive users require complete assurance that location and personal device details are purged before sharing files. Pure Rust implementation runs locally without network calls.

---

## M4 — Add BG + Remove BG + Edit

### D024: Add BG backdrop composition with gradient generation and diffused shadows
**Decision:** Implement native pixel compositing in `wheel-engines::bg` featuring customizable linear/radial gradients, corner radius masking, and soft drop shadows with aspect ratio constraints (Auto, 1:1, 16:9, 4:5).
**Rationale:** Enables immediate production-ready backdrop screenshot beautification locally without cloud uploads.

### D025: Non-destructive image adjustments with real-time CSS preview
**Decision:** Implement fast client-side CSS filter preview (`brightness`, `contrast`, `saturate`, `sepia`, `hue-rotate`) paired with backend pixel transforms (`wheel-engines::edit`) for rotation (90°, 180°, 270°), flips (H, V), and dimension resizing with aspect lock.
**Rationale:** Provides instant 60fps feedback while adjusting sliders in the UI, then applies high-quality Lanczos3 resampling upon export.

### D026: RMBG-1.4 model manager with streaming download and split comparison UI
**Decision:** Implement an on-demand model download manager for RMBG-1.4 (~176 MB) using `ureq` with streaming progress events, cached at `%LOCALAPPDATA%\Wheel\models\rmbg-1.4.onnx`, combined with an interactive Before/After split view slider.
### D027: Irreversible pixel burn redaction architecture
**Decision:** Overwrite pixel memory permanently in Rust (`wheel-engines::redact`) using pure black RGB(0,0,0), irreversible 16x16 block averaging (pixelation), or destructive heavy Gaussian blur. Never output reversible vector masks or CSS-obscured elements. Expose an explicit Amber/Red warning badge informing users that redactions are non-reversible and EXIF/location metadata is stripped upon export.
**Rationale:** Standard PDF/image annotation overlays frequently fail security audits because underlying pixels remain intact or recoverable. Direct raster memory destruction eliminates any possibility of recovery.

### D028: Full-resolution annotation compositing via lossless overlay streaming
**Decision:** Implement drawing tools (pen, semi-transparent highlighter, arrows, boxes, text, and numbered auto-incrementing step markers) on an HTML5 canvas synchronized to natural image pixels. The rasterized overlay is sent to `wheel-engines::annotate` as lossless PNG bytes and alpha-blended directly onto the source image buffer.
**Rationale:** Drawing on small screen previews often degrades quality when scaled back to multi-megapixel photos. Rendering coordinates at native natural dimensions ensures DPI-crisp vector rendering and perfect compositing fidelity regardless of camera sensor resolution.

### D029: FFmpeg execution security and multi-path discovery
**Decision:** Enforce absolute command-line safety for media conversions by passing strict argument arrays (`std::process::Command::arg(...)`) with zero shell string concatenation. Discover FFmpeg across three prioritized locations: `%LOCALAPPDATA%\Wheel\bin\ffmpeg.exe`, application directory sidecar, and system PATH. Implement high-fidelity GIF creation using a two-pass palette pipeline (`fps=15,scale=540:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse`).
**Rationale:** Defends against path injection or malicious file name execution while delivering clean GIF animation and high-fidelity video/audio transcoding.

### D030: Raycast-style Settings architecture and live schema persistence
**Decision:** Implement a modular, tabbed desktop settings window (`SettingsWindow.tsx`) with instant schema synchronization (`wheel_core::WheelSettings`), live SQLite history inspection, RMBG-1.4 model management, and FFmpeg capability detection.
**Rationale:** Ensures users have transparent control over background triggers, output naming policies, and local disk usage without opening raw JSON configuration files.

### D031: Elevation-free Windows 11 Explorer Context Menu integration
**Decision:** Register Wheel in the Windows Explorer context menu using `HKCU\Software\Classes\*\shell\Wheel` and `HKCU\Software\Classes\Directory\shell\Wheel` via `wheel_win::shell`.
**Rationale:** Standard system-wide context menu registration requires administrator UAC elevation and complex shell COM extensions. Registering under `HKCU` provides an instantaneous, one-click right-click menu entry on both Windows 10 and 11 without requesting administrative permissions.

### D032: Multi-step Action Preset chaining engine
**Decision:** Provide a sequential action chain runner in `commands::run_preset` that executes declarative recipes (e.g. Clean Web Asset: strip metadata -> convert to WebP; Cutout PNG: remove background -> convert to PNG). Intermediate outputs are isolated in temporary scratch paths and safely pruned after the terminal output is produced.
**Rationale:** Enables advanced automation workflows from a single drag-and-drop gesture or palette search command while guaranteeing that intermediate artifacts never clutter the user's workspace.

### D033: Spotlight/Raycast Command Palette floating surface
**Decision:** Implement a dedicated floating command palette (`640x480`, borderless, always-on-top) supporting fuzzy searching across all tools, conversions, presets, and SQLite history items, with keyboard navigation (`↑`/`↓`, `Enter`, `Esc`).
**Rationale:** Serves power users who prefer keyboard-centric workflows over mouse gestures and provides quick access to recent conversions and file operations.

### D034: Low-level keyboard Shift detection, OLE STA initialization, and background window lifecycle
**Decision:**
1. In `wheel_win::hooks`, detect `VK_LSHIFT` (`0xA0`) and `VK_RSHIFT` (`0xA1`) alongside `VK_SHIFT` (`0x10`), and back with `GetAsyncKeyState` in the low-level mouse procedure.
2. If the user presses Shift mid-drag while the mouse is already in motion, arm the trigger immediately without requiring a re-click.
3. Call `OleInitialize(None)` before `RegisterDragDrop` to guarantee STA COM initialization on the overlay thread.
4. Remove the default `"main"` window from `tauri.conf.json` (`"windows": []`), and configure `tauri_plugin_window_state` to ignore `"overlay"` and never restore `VISIBLE` state flags.
5. In `start_hook_listener`, check whether `WM_LBUTTONUP` occurs within the overlay bounds before emitting `drag-cancelled`, preventing premature cancellation while OLE `Drop` is being dispatched.
**Rationale:** In Windows, `WH_KEYBOARD_LL` sends specific left/right keycodes rather than generic `VK_SHIFT`, which caused Shift to appear unpressed. Without `OleInitialize`, Win32 OLE DragDrop registration fails or drops are rejected. Removing the default window from configuration eliminates the blank white 800x600 window and restores Wheel to a proper tray-only background architecture.

---

## Pending / Open Questions

- **Code signing:** Windows code signing requires a certificate. The Tauri bundler supports
  it via env vars. We'll document the process in M7 without purchasing one for dev builds.

