# Wheel — Architecture Overview

## System Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        WINDOWS OS                                │
│  ┌──────────────────┐    ┌──────────────────────────────────┐   │
│  │  Explorer / App  │    │        Win32 Hook Thread          │   │
│  │  (drag source)   │    │  WH_MOUSE_LL + WH_KEYBOARD_LL    │   │
│  └────────┬─────────┘    │  (lock-free channel → Tokio)      │   │
│           │ OLE DnD      └──────────────┬───────────────────┘   │
│           │                             │ WinEvent::DragArmed    │
│           ▼                             ▼                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │               WHEEL TAURI PROCESS (Rust)                │    │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐ │    │
│  │  │  Overlay Window   │  │         Tokio Runtime         │ │    │
│  │  │  (WebView2, hid)  │  │  ┌───────────────────────┐  │ │    │
│  │  │  ┌─────────────┐ │  │  │     Job Queue (4x)     │  │ │    │
│  │  │  │ IDropTarget │ │  │  │  spawn_blocking →      │  │ │    │
│  │  │  │ (OLE COM)   │ │  │  │  wheel-engines         │  │ │    │
│  │  │  └──────┬──────┘ │  │  └───────────────────────┘  │ │    │
│  │  └─────────┼────────┘  │                               │ │    │
│  │            │ Events    │  ┌───────────────────────┐    │ │    │
│  │            ▼           │  │   Tauri Commands      │    │ │    │
│  │  ┌─────────────────┐   │  │  get_actions          │    │ │    │
│  │  │ Tauri emit()    │   │  │  dispatch_action      │    │ │    │
│  │  │ → WebView2 JS   │   │  │  show/hide_overlay    │    │ │    │
│  │  └─────────────────┘   │  └───────────────────────┘    │ │    │
│  │                         └──────────────────────────────┘ │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                ▲                                  │
│           Tauri IPC (invoke)   │                                  │
│                                ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              WEBVIEW2 FRONTEND (React/TS)               │    │
│  │  ┌─────────────────────┐  ┌──────────────────────────┐  │    │
│  │  │   WheelOverlay.tsx  │  │   Tool Windows (M3+)      │  │    │
│  │  │   RadialWheel.tsx   │  │   Crop / Compress / ...   │  │    │
│  │  │   Zustand Store     │  └──────────────────────────┘  │    │
│  │  │   Motion animations │                                 │    │
│  │  └─────────────────────┘                                 │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow: Shift + File Drag

```
1. User holds Shift + left-click-drags a file in Explorer
2. WH_MOUSE_LL sees movement > threshold while Shift is down
   → pushes WinEvent::DragArmed{x, y} to Tokio channel
3. Tauri async handler:
   a. Reads settings (paused? threshold?)
   b. Calls clamp_to_work_area() for DPI-aware positioning
   c. Sets overlay window position
   d. Shows overlay window
   e. Emits "drag-armed" to WebView2
4. WebView2 receives "drag-armed":
   a. Zustand store: isDragging = true
   b. Motion animates wheel in (spring, 120-180ms)
5. Explorer OLE starts DragEnter on overlay HWND
   a. IDropTarget::DragEnter extracts CF_HDROP paths
   b. Returns DROPEFFECT_COPY (NEVER DROPEFFECT_MOVE)
   c. Emits "drop-enter" with files + extensions to WebView2
6. WebView2 filters wedges by accepted extensions
7. User moves cursor over wedge → OLE DragOver events → hit-test
8. User drops:
   a. IDropTarget::Drop fires
   b. Emits "drop-files" to WebView2 with wedge id
   c. WebView2 calls invoke("dispatch_action")
   d. Rust: instant → JobQueue → spawn_blocking → wheel-engines
           window  → open tool WebviewWindow
9. Overlay hides; toast notification on completion
```

## Crate Responsibilities

| Crate | Responsibility | Windows deps |
|-------|---------------|--------------|
| `wheel-core` | Action registry, Job queue, Settings schema, Output naming | ❌ (testable) |
| `wheel-win` | WH_MOUSE_LL/KB hooks, IDropTarget, DPI helpers, vibrancy | ✅ |
| `wheel-engines` | Image/PDF/video conversion engines | ❌ |
| `apps/desktop/src-tauri` | Tauri bootstrap, window mgmt, tray, commands | via wheel-win |

## Key Design Decisions

See `DECISIONS.md` for rationale. Summary:
- Pre-created hidden overlay → <50ms show time
- OLE IDropTarget (not Tauri's built-in) → proper file path extraction + COPY effect
- Tokio job queue with semaphore → backpressure, cancellation, history
- Zustand + Tauri events → reactive UI without polling
- SVG wedge geometry → resolution-independent, no canvas overhead for the ring
