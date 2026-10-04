# Lime

Lime is a desktop app for Windows that helps you quickly convert, process, and optimize images using a radial drag-and-drop menu.

When you drag image files and hold the **Shift** key, a wheel appears at your cursor. Drop your files onto any section of the wheel to convert formats or run image processing tools.

[**Download Latest Release (Windows)**](https://github.com/muneebbug/lime/releases)

---


## Features

- **Drag-and-Drop Wheel**: Hold Shift while dragging files to bring up the wheel right under your cursor.
- **Image Conversions**: Convert images to PNG, JPG, WebP, AVIF, TIFF, BMP, ICO, and PDF. Works with common image formats including SVG, PNG, JPG, WebP, AVIF, TIFF, BMP, and GIF.
- **Built-in Tools**: Handy image utilities right in the wheel (such as automatically trimming blank or transparent borders, with more tools being added).
- **Sound Effects**: Audio clicks when hovering over wheel options, which can be turned on or off in settings.
- **Settings & System Tray**: Customize trigger keys, wheel size, audio effects, and update channels, or pause the app from the taskbar tray.
- **Private and Offline**: All processing happens locally on your computer. No files or data ever leave your machine.

---

## TODO

- Add Hardware Acceleration support for media conversions for NVIDIA, AMD and Intel GPUs.
- Add support for converting PDF to images
- Optimize Video to GIF conversion file sizes (Current GIF files sizes are huge).
- Implement Jpeg/Png to Tiff Exif preservation and vice versa.

## How to Use

1. Select one or more image files in File Explorer.
2. Begin dragging the files, then press and hold the **Shift** key.
3. The wheel will appear around your mouse cursor.
4. Move your mouse over your desired action (such as a conversion format or a tool like **Trim**).
5. Release the mouse button. The processed files are saved in the same folder next to the originals.

---

## Requirements

- Windows 10 or 11 (64-bit)
- [Node.js](https://nodejs.org/) (v20+) and [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/) (1.77+)

---

## Development

Install dependencies:

```powershell
pnpm install
```

Start the app in development mode:

```powershell
pnpm dev
```

Run test suites:

```powershell
cargo test --workspace
```

Build the production installer:

```powershell
pnpm build
```

The installer executable will be generated in `apps/desktop/src-tauri/target/release/bundle/nsis/`.

---

## Settings & Storage

Lime stores your preferences and past conversion history locally at `%LOCALAPPDATA%\Lime\`:
- `settings.json`: Configuration for hotkeys, thresholds, and appearance.
- `history.db`: Local database of your recent conversions.

You can open the settings window at any time by right-clicking the Lime tray icon and clicking **Open Settings**.
