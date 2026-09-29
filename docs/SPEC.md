<USER_REQUEST>
# Agent Prompt: Build "Wheel" — a Windows radial drag-and-drop file toolkit

You are a senior Windows desktop engineer (Rust + TypeScript). Scaffold and build a production-quality Windows 10/11 app end to end. Work autonomously: make decisions, document them in `docs/DECISIONS.md`, and only stop to ask me a question if you are truly blocked. Build in the milestones defined below and verify each milestone's acceptance criteria before moving on.

The working name is **Wheel**. Keep the name in one config constant so it is easy to rename.

---

## 1. Product summary

Wheel is a background tray app. When the user **holds Shift and drags one or more files**, a **radial wheel** appears centered on the cursor. The user drops the file onto a wedge of the wheel. Each wedge is either:

- a **Convert** target (PNG, WEBP, AVIF, TIFF, PDF, DOCX, BMP, ICO, JPG, and so on), which runs instantly in the background, or
- a **Tool** (Crop, Add BG, Compress, Redact, Metadata, Edit, Annotate, Remove BG, and so on), which opens in **its own dedicated window** for that tool.

The app must feel native to Windows 11 (Mica/Acrylic, rounded corners, system accent color, dark/light mode), be extremely fast to appear (<50 ms from trigger to first frame), and be **highly configurable in the way the Raycast for Windows app is configurable** (see section 9).

Reference: the design is inspired by a macOS app with a glassy, warm-gradient radial menu. Reference screenshots of the wheel and of the Crop, Add BG and Compress windows accompany this prompt. If you cannot view them, rely on the UI descriptions in section 7.

---

## 2. Tech stack, versions and scaffolding policy

### 2.0 Latest-versions policy (applies to EVERYTHING)
- **Use the latest stable release of every dependency, toolchain and sidecar.** Do not rely on version numbers written in this prompt or remembered from training data; they may be outdated. Look them up at the moment you scaffold.
- How to check: `rustup update stable` and `rustc --version`; `cargo search <crate> --limit 1` or `cargo info <crate>` (or crates.io) for Rust crates; `npm view <package> version` (or `pnpm view`) for JS packages; the official release pages for Node LTS, pnpm, WebView2, ONNX Runtime, FFmpeg, ExifTool and PDFium builds.
- Install with the CLI's own tooling so the manifest gets the newest compatible version: `cargo add <crate>`, `pnpm add <pkg>@latest`, `pnpm tauri add <plugin>`. Never hand-write version strings from memory.
- Keep **all Tauri crates and their JS counterparts on matching versions** (`tauri`, `tauri-build`, `@tauri-apps/api`, `@tauri-apps/cli`, and every `tauri-plugin-*` with its `@tauri-apps/plugin-*` package). Tauri warns on mismatches; resolve them.
- Use the **latest stable Node.js (LTS)** and **pnpm**; the **Rust stable toolchain with the `x86_64-pc-windows-msvc` target**; and the current **Rust edition** used by the official Tauri template.
- After the scaffold, run `cargo update` and `pnpm update --latest`, then build and test. If a "latest" release is a breaking major that blocks progress, pin the previous version, record the exact reason and versions in `docs/DECISIONS.md`, and add a TODO to upgrade. Never use pre-release, alpha, beta or RC versions unless no stable exists for something required (record it).
- Generate `docs/VERSIONS.md` at the end of M0 listing every direct dependency with the resolved version and the date it was checked. Add a `pnpm deps:check` / `cargo outdated`-style script, and configure Renovate or Dependabot for weekly updates.
- APIs change between releases. **Read the current official docs and crate/package docs (docs.rs, npm readmes, changelogs) for the version you actually installed before writing code against it**, especially for `tauri`, `windows`, `ort`, `window-vibrancy`, `pdfium-render`, `react-konva` and `framer-motion` (now published as `motion`; check the current package name).

### 2.1 Stack (decided; do not re-litigate unless blocked)

- **Shell/framework:** the latest stable **Tauri** (the 2.x line at the time of writing; confirm from the official docs), Rust backend, WebView2 frontend. Use official Tauri plugins wherever one exists (updater, single-instance, autostart, notification, global-shortcut, window-state, store/fs/shell/opener/process as needed) rather than hand-rolling.
- **Frontend:** React + TypeScript + Vite, Tailwind CSS, Motion (formerly Framer Motion; use the current package) for wheel animation, Zustand for state, Radix UI primitives for accessible controls, Konva (react-konva) for canvas tools (crop overlay, annotate, redact).
- **Native layer (Rust):** the `windows` crate (Win32, COM/OLE, Shell, DWM, UI Automation where needed), `tokio`, `serde`, `tracing`.
- **Window effects:** `window-vibrancy` crate (Mica / Acrylic / Tabbed) with fallbacks.
- **Image engines:** `image` + `fast_image_resize`, `resvg` for SVG, WIC (via `windows` crate) as a fallback for AVIF when native crates fail, `pdfium-render` for PDF rasterization, `printpdf` or `lopdf` for image to PDF, `docx-rs` for image to DOCX.
- **Background removal:** ONNX Runtime (`ort` crate) with DirectML execution provider (CPU fallback). Use an open model (RMBG-1.4 or U2-Net). **Download the model on first use** into `%LOCALAPPDATA%\Wheel\models` with a progress UI and SHA-256 verification; do not bundle it in the installer.
- **Video/audio:** FFmpeg as a Tauri **sidecar** binary (download on first use or bundle, your call; document the decision), with hardware encoders (NVENC/QSV/AMF) auto-detected and used when available.
- **Metadata:** ExifTool sidecar for read/write/strip across formats (fallback to `nom-exif`/`kamadak-exif` for read-only if ExifTool is missing).
- **Persistence:** JSON settings in `%APPDATA%\Wheel\settings.json` with a versioned schema and migrations; SQLite (`rusqlite`) for job history and usage stats.
- **Packaging:** Tauri bundler producing an MSI/NSIS installer, per-user install by default (no admin required), auto-update via the Tauri updater plugin.

If any dependency proves unworkable, pick the closest alternative and record why in `DECISIONS.md`.

---

## 3. Scaffolding: follow the official Tauri docs

**Do not invent the project structure from memory. Scaffold with the official tooling and docs first.**

1. Read the current official docs before running anything: **https://v2.tauri.app/start/prerequisites/** (Windows prerequisites: Microsoft C++ Build Tools with the "Desktop development with C++" workload, WebView2 runtime, Rust via rustup with the MSVC toolchain) and **https://v2.tauri.app/start/create-project/** (project creation). Also consult, as needed, the docs on configuration files, the **capabilities/permissions system**, window customization (transparent/decorations/always-on-top/skip-taskbar), calling Rust from the frontend (commands, events, channels), sidecars (external binaries), state management, the plugin list, and **Windows installer/bundling and code signing**. If the docs and this prompt disagree, the docs win; record the difference in `DECISIONS.md`.
2. Verify prerequisites on this machine (`rustc -V`, `cargo -V`, `node -v`, `pnpm -v`, WebView2 present). Install or update whatever is missing or outdated, following the docs.
3. Scaffold using the official `create-tauri-app`, with the **latest** release, non-interactively so it is reproducible:
   `pnpm create tauri-app@latest` (run it inside `apps/`, choosing project name `desktop`, identifier `com.<yourorg>.wheel`, language TypeScript/JavaScript, package manager pnpm, template **React**, flavor **TypeScript**). Use the interactive prompts if the flags differ in the current release; check `pnpm create tauri-app --help` and the create-project docs.
4. Run the scaffolded app as generated (`pnpm tauri dev`) and confirm the default window works **before changing anything**. Commit that as the baseline.
5. Only then adapt it into the monorepo below: add the root Cargo workspace (with `apps/desktop/src-tauri` as a member), the `crates/*` members, and the pnpm workspace file, keeping the official template's conventions (`tauri.conf.json`, `capabilities/`, `src-tauri/icons`, Vite config). Add Tailwind, Zustand, Radix, Konva and Motion using each project's **current official installation guide** (for example, Tailwind's Vite plugin setup as documented now, not a legacy PostCSS recipe).
6. Add Tauri plugins with `pnpm tauri add <plugin>` so both the Rust crate and JS package are installed and registered correctly, then grant only the permissions needed in `capabilities/*.json`.
7. Configure additional windows (overlay, tool windows, settings, palette) using the documented window configuration and `WebviewWindowBuilder`, and verify each option (transparency, `alwaysOnTop`, `skipTaskbar`, `decorations`, `shadow`, focus behavior, `dragDropEnabled`) against the current docs and window APIs, because some are platform-specific or have changed between releases.

## 4. Repository layout (after step 5 above)

Create a monorepo:

```
wheel/
  Cargo.toml                  # workspace
  package.json / pnpm-workspace.yaml
  apps/
    desktop/                  # Tauri app
      src-tauri/              # Rust: tauri commands, window mgmt, tray
      src/                    # React frontend
        windows/
          overlay/            # the radial wheel
          tools/<tool-id>/    # one folder per tool window
          settings/           # settings app
          palette/            # command palette (see section 9)
        lib/ ui/ store/
  crates/
    wheel-core/               # action registry, job queue, settings schema, output naming
    wheel-win/                # ALL Win32/OLE/shell code (hooks, drop target, shell integration)
    wheel-engines/            # image, pdf, video, bg-removal, metadata engines
  docs/
    ARCHITECTURE.md  DECISIONS.md  ACTIONS.md  TESTING.md
  scripts/                    # dev, build, sidecar fetch, lint
```

Rules: all unsafe Win32 code lives in `wheel-win` behind safe wrappers with doc comments. `wheel-core` has no Windows dependencies so it stays unit-testable.

---

## 5. The hardest part: Shift + file-drag trigger and the drop-target overlay

Windows has no "file drag started" event, so implement this carefully. Build and validate this in **Milestone 0** before anything else.

### 5.1 Detection
1. Run a dedicated thread with a Win32 message pump that installs **low-level hooks**: `WH_MOUSE_LL` and `WH_KEYBOARD_LL`. Hook callbacks must return immediately (Windows silently removes slow hooks). Do nothing but push events onto a lock-free channel; process them on another thread.
2. Track state: `shift_down`, `lbutton_down`, cursor position, distance moved since button-down.
3. **Arming rule:** when `lbutton_down` and movement exceeds a threshold (default 6 px, configurable) **and** the configured modifier (default Shift) is held, treat it as a *possible file drag*.
4. **Confirming it is a file drag:** show the overlay immediately (pre-created, hidden) and let the overlay's OLE drop target confirm via `DragEnter` that the data object contains `CF_HDROP` / `CFSTR_SHELLIDLIST`. If no valid file data arrives within ~250 ms (configurable), hide the overlay silently. This avoids fragile heuristics about the source app.
5. Also allow the trigger to be configured as: modifier + drag (default Shift), no modifier (always show on file drag), or hotkey-only.
6. Sources that must work: Explorer (windows and desktop), browsers dragging downloaded files, Outlook attachments, Teams/Slack downloads, and virtual file drops (`CFSTR_FILEDESCRIPTOR` / `CFSTR_FILECONTENTS`, which must be materialized to a temp file before processing).

### 5.2 Overlay window
- A **pre-created, hidden** always-on-top window, sized larger than the wheel (for hover halo and animation), fully transparent, borderless, no taskbar entry.
- Extended styles: `WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED` (or per-pixel alpha via DWM/WebView2 transparency, whichever works reliably with the current Tauri release; document what you chose). It must **never steal focus** from the drag source.
- Position: centered on the cursor, clamped to the current monitor's work area, DPI-aware per monitor (Per-Monitor V2).
- Show/hide with a fast animation (scale + fade, spring, 120 to 180 ms). Hide instantly when the drag ends.

### 5.3 Drop target
- Implement a custom **`IDropTarget`** on the overlay HWND (`RegisterDragDrop`), and **disable Tauri's built-in drag-drop handling** for that window (`dragDropEnabled: false`) so they do not conflict.
- Forward `DragEnter/DragOver/DragLeave/Drop` to the frontend as events with cursor coordinates so the webview can highlight the wedge under the cursor. Do hit-testing on the frontend using the wheel geometry; send the resolved wedge id back on drop.
- **Important:** holding Shift during a drag makes Explorer default to *Move*. Always return `DROPEFFECT_COPY` (or `DROPEFFECT_LINK`) from the drop target so the source file is never moved or deleted by accident.
- Support multiple files. Actions declare whether they accept multi-file input; tools that accept only one file use the first file and tell the user.
- Handle the file-type context: after `DragEnter`, inspect extensions/MIME and send a `context` event so the wheel can show only relevant wedges (see 7.1).

### 5.4 Edge cases to handle and document
- Drag started in an **elevated (admin) app**: low-level hooks from a non-elevated process still see input, but OLE drops across integrity levels may be blocked. Detect and show a one-time explanation with an option to run Wheel elevated.
- Multi-monitor with mixed DPI, fullscreen apps/games (do not show over exclusive fullscreen; configurable), Remote Desktop, Esc to cancel, touch/pen input, and rapid repeated drags.
- Ensure hooks are removed and the overlay is hidden on sleep/resume, session lock, and app exit.
- A kill switch: tray menu "Pause Wheel" and a global hotkey to toggle it.

**Milestone 0 acceptance:** Shift+drag a file from Explorer shows a transparent wheel at the cursor in under 50 ms without stealing focus; hovering highlights wedges; dropping reports the correct file path(s) and wedge id in the log; the source file is untouched; releasing outside the wheel hides it.

---

## 6. Action architecture (make adding tools trivial)

Every wedge and tool is an **Action** described by a manifest, registered in `wheel-core`:

```jsonc
{
  "id": "tool.crop",
  "title": "Crop",
  "icon": "crop",                       // from the icon set, or a path
  "category": "tools",                  // "convert" | "tools" | custom
  "accepts": { "extensions": ["png","jpg","jpeg","webp","ico","bmp","tiff","avif","pdf"], "multi": false },
  "kind": "window",                     // "instant" (runs in background) | "window" (opens a tool window)
  "window": { "width": 640, "height": 860, "resizable": true, "mica": true },
  "defaults": { }
}
```

- Rust trait `Action { fn manifest() -> Manifest; async fn run(ctx, input, params, progress) -> Result<Output> }`.
- A central **job queue** (tokio) with progress events, cancellation, concurrency limits, and history in SQLite. Tool windows submit jobs; instant conversions submit jobs directly.
- **Output policy** (configurable, default "save next to the source"): `name.ext` becomes `name.converted.ext` or `name (1).ext` on collision; never overwrite the source unless the user explicitly enables it; options for a fixed output folder, ask-each-time, or copy result to clipboard; optional "send source to Recycle Bin after success" using `IFileOperation`.
- **Notifications:** on completion show a native Windows toast (action buttons: Open, Show in Folder, Copy path, Undo where possible). Show taskbar progress for long jobs.
- Adding a new tool must require: one manifest, one Rust `Action` impl (or a sidecar command), and one React folder for its window. Document the steps in `docs/ACTIONS.md` with a worked example.

---

## 7. UI specification

### 7.1 Radial wheel
- Circular ring with 6 to 10 wedge slots plus a **center pill**. Wedges are rounded, glassy, semi-translucent tiles with a subtle inner highlight, thin outer ring and soft shadow, tinted by a configurable gradient (default warm orange to coral, as in the reference), following system dark/light mode.
- Each wedge shows an icon (tools) or a bold label (formats). The wedge under the cursor lifts, brightens and switches to the accent color (a saturated red-orange in the reference).
- The **center pill shows the label of the hovered wedge** (for example "TIFF", "ANNOTATE") and the file count/name while idle.
- **Two pages:** *Convert* (format wedges) and *Tools* (tool wedges). Switch pages by: dwelling on the center pill, pressing `Tab`, `Space`, or the mouse wheel during the drag. Show small page dots. Remember the last-used page (configurable).
- **Context-aware wedges:** only show actions that accept the dragged files (do not offer image output for a video). Same-format conversions are hidden. If a page has fewer wedges than slots, re-space evenly.
- Slot count, order, and contents are user-configurable (section 9). Also support number keys `1` to `9` to pick a wedge while dragging.
- Sound and haptics are optional and off by default.

### 7.2 Tool windows (each opens in its own frameless window)
Common chrome for **every** tool window, matching the reference:
- Rounded (Win11-style) frameless window with Mica/Acrylic and a warm gradient tint, opening centered on the monitor where the drop happened.
- Header: circular **close (X)** button on the left, **centered title**, thin divider below.
- Footer: secondary **Reset** button on the left, primary accent **action button** on the right (Apply / Save with Background / Compress).
- Escape closes; Enter triggers the primary action; standard Windows window snapping still works; remembers size/position per tool (configurable); several tool windows can be open at once.
- Show a progress state on the primary button while running, then a success state with "Open / Show in folder".

Tool-specific requirements:

1. **Crop Image** (image and PDF page): live preview with draggable corner and edge handles (react-konva). *Aspect ratio* segmented control: **Free, 1:1, 16:9, 9:16, 4:3, 3:4** (plus custom). **W** and **H** pixel inputs that stay in sync with the handles and ratio lock. Reset, Apply.
2. **Add BG** (drop a backdrop behind an image or screenshot): live preview; a **Background** picker with two rows of swatches (gradient presets and solid colors), plus a "use custom image" button and a custom color picker; sliders for **Padding (px)**, **Corners (px)** and **Shadow (px)** with numeric readouts; **Ratio** segmented control **Auto, 1:1, 16:9, 4:5**; button **Save with Background**. Export as PNG/JPG/WEBP.
3. **Compress** (images, PDFs, video, audio): shows filename and original size; **Compression preset** segmented control (**Balanced**: smaller files with less quality loss, **Strong**: smallest size); a **"Compress to target file size"** checkbox that reveals a size input and picks parameters by search (for video use two-pass bitrate targeting; for images binary-search quality); shows estimated result size when possible; **Compress** button. Preserve the original; write a new file.
4. **Remove BG:** runs the ONNX model, shows before/after with a draggable split slider, optional edge refinement/feathering slider, and background options (transparent, solid color). Export PNG/WEBP with alpha. Handle first-run model download gracefully.
5. **Metadata:** a searchable, grouped viewer (File, EXIF, GPS, XMP, IPTC, Video/Audio, PDF) with copy-value support; toggles/buttons to **strip all**, **strip GPS only**, or edit selected editable tags; save as a cleaned copy. Show a map link for GPS but never load remote content automatically.
6. **Redact:** draw boxes/brush over regions with solid fill or pixelate/blur. For images, burn redactions into pixels irreversibly. For PDFs, rasterize redacted pages (or truly remove text objects) so hidden text cannot be recovered; state which mode was used. Include a warning for non-flattened output.
7. **Annotate:** pen, highlighter, arrows, rectangles, ellipses, text, numbered markers, blur; color and stroke width controls; undo/redo; copy to clipboard or save.
8. **Edit:** adjustment sliders (brightness, contrast, saturation, exposure, sharpness, temperature), rotate/flip, resize with aspect lock, with live preview and Reset.

Add more tools only via the action architecture. Build in this order: Convert, Crop, Compress, Metadata, Add BG, Remove BG, Edit, Annotate, Redact.

### 7.3 Conversion matrix (instant wedges)
- Image to image: PNG, JPG, WEBP, AVIF, TIFF, BMP, GIF (static), ICO.
- Image to PDF (single or multiple images merged into one PDF, page size options), PDF to PNG/JPG (per page, zip if many), image/PDF to DOCX (embed as image pages; be honest in the UI that this is not OCR unless OCR is enabled).
- Optional OCR (Windows.Media.Ocr or Tesseract) as a later tool: "Image/PDF to searchable text or DOCX".
- Video/audio conversions via FFmpeg (MP4, MOV, WEBM, MKV, GIF, MP3, WAV, M4A, FLAC).
- Preserve metadata by default, with a setting to strip. Respect EXIF orientation and ICC profiles. Batch conversion with a combined progress toast.
- Clearly report unsupported combinations before the drop (hide the wedge) instead of failing after.

---

## 8. Deep Windows integration checklist

Implement all of these, each behind a setting where sensible:

- **Tray icon** with menu: Pause/Resume, Open Settings, Open Palette, Recent Jobs, Check for Updates, Quit.
- **Start with Windows** (per-user startup registration; start minimized to tray).
- **Single instance** with argument forwarding (`wheel.exe --tool crop "C:\path\file.png"` opens that tool directly).
- **Explorer context menu:** a "Wheel" submenu using a Windows 11 modern `IExplorerCommand` handler, plus a classic-menu fallback (registry verbs under `SystemFileAssociations`). Scope this as its own milestone (M6); the app must work fully without it.
- **Send To** shortcut installation option.
- **Selected-files acquisition** from Explorer for the palette (via `IShellWindows`/UI Automation or the clipboard fallback), so "run a tool on what I have selected" works.
- **Mica/Acrylic** backdrops, rounded corners (`DWMWA_WINDOW_CORNER_PREFERENCE`), immersive dark mode title handling, system accent color, and reduced-motion / high-contrast settings respected.
- **Per-Monitor V2 DPI**, multi-monitor placement, taskbar progress, native toast notifications with actions, jump list entries (Recent, Settings).
- **Protocol handler** `wheel://run?tool=crop&file=...` for automation.
- **Drag-out:** after a job finishes, allow dragging the result file out of the toast/history entry into other apps.
- **Clipboard:** actions that copy the result to the clipboard as both an image and a file.
- **Security:** least-privilege Tauri capabilities, strict CSP, validated paths (no shell string concatenation; always pass argument arrays to sidecars), temp files created in a private folder and cleaned up, no telemetry by default (opt-in only).
- **Installer:** per-user MSI/NSIS via the Tauri bundler as described in the current official Windows installer and code-signing docs, WebView2 bootstrapper, uninstaller that removes hooks/registry entries.

---

## 9. Configurability (Raycast-for-Windows style)

Build a proper **Settings app** (its own window) with a left sidebar and searchable settings. Treat every wedge, tool, and hotkey as a first-class configurable item.

**Sidebar sections:**
1. **General:** launch at login, tray behavior, language, auto-update channel, output folder/naming policy, delete-source policy, notification style.
2. **Trigger:** modifier key choice (Shift default, Ctrl, Alt, none), movement threshold, confirm timeout, per-app allow/deny list (for example never trigger in games or specific apps), fullscreen behavior, global hotkey to pause.
3. **Wheel:** slot count, page layout, drag-to-reorder wedges, enable/disable each action, pin favorites to the first page, context-aware filtering, size, corner radius, theme (System/Light/Dark), gradient/accent presets, animation speed and reduced-motion, sound/haptics.
4. **Actions/Tools:** list of all actions with enable toggle, icon, per-action default parameters (for example default compression preset, default crop ratio, default output format and quality), per-action global hotkey, and per-action aliases for the palette.
5. **Formats:** per-format defaults (quality, metadata handling, color profile), and which formats appear in the Convert page.
6. **Hotkeys:** all shortcuts in one place with conflict detection and recording UI.
7. **Command palette:** a global hotkey (default `Ctrl+Alt+Space`, user-changeable) that opens a fast fuzzy-search launcher listing actions and recent files; select an action, then pick or paste a file (or use the current Explorer selection). Support aliases, recent-first ranking, and keyboard-only flow. Add "quicklink" style presets, for example "Compress to under 2 MB" as saved parameterized actions.
8. **Presets/Workflows:** user-defined presets ("WebP 80% + strip metadata", "Screenshot backdrop") that appear as wedges or in the palette; simple chains of actions (for example Remove BG then Add BG then Export).
9. **Extensions (v1 minimal):** a folder-based plugin system where a plugin is a manifest plus a command-line executable or script that receives file paths and returns output paths; discoverable from Settings; sandboxed by design (explicit permissions listed in the manifest). Document the format in `docs/ACTIONS.md`. A full extension store is out of scope.
10. **Advanced:** import/export all settings as JSON, reset to defaults, diagnostics log viewer, "copy debug info", open data folder, model and sidecar manager (download, verify, delete).
11. **About:** version, changelog, licenses.

Requirements: settings apply live without restart, are validated against a versioned schema with migrations, and the whole config can be edited as JSON for power users.

---

## 10. Performance, reliability and quality bars

- Overlay cold-show under 50 ms (pre-created and pre-warmed webview); idle CPU near 0%; idle memory target under 150 MB including WebView2.
- Never block the UI thread or hook thread. Heavy work runs in the job queue; use `spawn_blocking` for CPU-bound engines.
- Crash safety: hooks and windows torn down in `Drop`; a watchdog re-installs hooks if Windows removes them; panics are caught and logged with `tracing` to `%LOCALAPPDATA%\Wheel\logs` (rotated).
- Large files: stream where possible; cancellable jobs; clear errors instead of silent failures; never leave partial output files (write to temp then atomic rename).
- Accessibility: the settings and tool windows are fully keyboard navigable with proper ARIA roles; the wheel supports keyboard selection in palette/hotkey mode; respect reduced motion and high contrast.
- Tests: unit tests for `wheel-core` (naming, presets, settings migrations), golden-image tests for engines, integration tests for actions on fixture files, and a manual test matrix in `docs/TESTING.md` covering sources (Explorer, desktop, browser, Outlook), DPI/monitor setups, and elevated apps.
- CI: GitHub Actions on `windows-latest` running `cargo fmt/clippy/test`, `pnpm lint/typecheck/test`, and producing an installer artifact.

---

## 11. Milestones (do them in order; report after each)

- **M0 Spike:** official-docs scaffold (section 3) verified running with the latest stable versions, `docs/VERSIONS.md` generated, then tray icon, hooks, pre-created transparent overlay, custom `IDropTarget`, Shift+drag trigger, correct paths and wedge id logged. (Acceptance in 5.4.)
- **M1 Wheel UI + core:** action registry, job queue, settings persistence, real wheel with two pages, context-aware wedges, hover and center-pill behavior, animations.
- **M2 Conversions:** image to image, image to PDF, PDF to image, output policy, toasts, history.
- **M3 Tool windows framework + Crop + Compress + Metadata:** shared window chrome (Mica, header, footer), per-tool window management, first three tools complete to spec.
- **M4 Add BG + Remove BG + Edit:** including ONNX model download manager and DirectML.
- **M5 Annotate + Redact + video/audio via FFmpeg.**
- **M6 Settings app + command palette + presets + Explorer context menu + extensions v1.**
- **M7 Hardening and release:** installer, auto-update, code-signing docs, accessibility pass, performance profiling, README with GIFs, full `docs/`.

After each milestone: run the app, exercise the acceptance criteria yourself, fix defects, update docs, commit with a clear message, and give me a short status (what works, what is risky, what is next).

---

## 12. Constraints and non-goals

- Windows 10 (1809+) and Windows 11 only; x64 first, ARM64 later.
- No cloud dependency for core features; everything runs locally. Network is used only for model/sidecar downloads and updates, and users can disable it.
- Do not copy assets, icons, names or code from the macOS reference app. Design an original icon set and visual identity inspired by the *interaction pattern* only.
- Do not fake features: if a conversion or tool has limits (for example PDF to DOCX without OCR), say so in the UI.

Begin with M0. First read the official Tauri docs listed in section 3 and verify prerequisites, then scaffold with the latest `create-tauri-app` and confirm the baseline runs. Next, produce `docs/ARCHITECTURE.md` (a one-page diagram and data flow for hooks, overlay, drop target, action registry and job queue), adapt the scaffold into the monorepo, and implement the spike.
</USER_REQUEST>
<ADDITIONAL_METADATA>
The current local time is: 2026-09-28T22:31:49+05:00.
</ADDITIONAL_METADATA>
<USER_SETTINGS_CHANGE>
The user changed setting `Model Selection` from None to Claude Sonnet 4.6 (Thinking). No need to comment on this change if the user doesn't ask about it. If reporting what model you are, please use a human readable name instead of the exact string.
</USER_SETTINGS_CHANGE>