# Wheel — Adding Actions Guide

This guide explains how to add new conversions (instant) or tools (window) to Wheel.

## Anatomy of an Action

Every action has three parts:
1. **Manifest** (in `wheel-core/src/action.rs` `default_actions()`)
2. **Engine** (in `wheel-engines` or as a sidecar command)
3. **UI** (a React folder under `src/windows/tools/<tool-id>/` for tools; none for instant)

---

## Example: Adding a New Convert Action (instant)

### Step 1: Add the manifest

In `crates/wheel-core/src/action.rs`, in `default_actions()`:

```rust
actions.push(ActionManifest {
    id: "convert.gif".to_string(),
    title: "GIF".to_string(),
    icon: "format-gif".to_string(),
    category: ActionCategory::Convert,
    accepts: AcceptedInput {
        extensions: vec!["png".into(), "jpg".into(), "webp".into()],
        multi: false,
    },
    kind: ActionKind::Instant,
    window: None,
    defaults: serde_json::json!({ "fps": 10 }),
    enabled: true,
    order: 8,
});
```

### Step 2: Handle it in `run_instant_action`

In `apps/desktop/src-tauri/src/commands.rs`, the `run_instant_action` function
uses the action_id suffix to look up the format. Add GIF support to `wheel-engines`:

```rust
// crates/wheel-engines/src/image_convert.rs
pub enum OutputFormat {
    // ... existing
    Gif,
}
```

### Step 3: That's it for instant actions!

The wheel automatically shows the new wedge on the Convert page.

---

## Example: Adding a New Tool Action (window)

### Step 1: Add the manifest

```rust
actions.push(ActionManifest {
    id: "tool.ocr".to_string(),
    title: "OCR".to_string(),
    icon: "ocr".to_string(),
    category: ActionCategory::Tools,
    accepts: AcceptedInput {
        extensions: vec!["png", "jpg", "jpeg", "pdf"],
        multi: false,
    },
    kind: ActionKind::Window,
    window: Some(WindowConfig {
        width: 720,
        height: 640,
        resizable: true,
        mica: true,
    }),
    defaults: serde_json::json!({ "language": "eng" }),
    enabled: true,
    order: 10,
});
```

### Step 2: Create the tool window React folder

```
src/windows/tools/tool-ocr/
  index.tsx          ← React root component
  OcrTool.tsx        ← Main tool UI
```

The URL will be `index.html?window=tool&tool=tool.ocr&files=[...]`.
Add a route in `src/App.tsx`:
```tsx
case "tool":
  const toolId = params.get("tool");
  if (toolId === "tool.ocr") return <OcrTool />;
```

### Step 3: Implement the engine

Add a `crates/wheel-engines/src/ocr.rs` module with the processing logic.
Handle it in `commands.rs::run_instant_action` if it can run without a window.

### Step 4: Register the Tauri command if needed

Tool windows call `invoke("dispatch_action")` on their primary button,
which already routes through the job queue.

---

## Extension Plugins (v1)

A minimal plugin is a folder with:
- `wheel-plugin.json` — manifest
- `plugin.exe` or `plugin.ps1` — the executable

```jsonc
{
  "id": "my.watermark",
  "title": "Add Watermark",
  "version": "1.0.0",
  "accepts": { "extensions": ["png", "jpg"], "multi": true },
  "command": "./watermark.exe",
  "args": ["--input", "{file}", "--output", "{output}"],
  "permissions": ["read-files", "write-files"]
}
```

The plugin receives file paths via arguments and must print the output paths to stdout.

See `docs/DECISIONS.md` for the sandboxing rationale.
