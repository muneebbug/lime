import { useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "motion/react";
import { useWheelStore } from "../../store/wheelStore";
import { RadialWheel, hitTestWedge, filterActions } from "./RadialWheel";

interface DragArmedEvent {
  x: number;
  y: number;
}

interface DropEnterEvent {
  files: string[];
  extensions: string[];
  x: number;
  y: number;
}

interface DropFilesEvent {
  files: string[];
  x: number;
  y: number;
}

export function WheelOverlay() {
  const store = useWheelStore();
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    store.loadActions();
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    const handleFilesDropped = (files: string[], x: number, y: number) => {
      const state = useWheelStore.getState();
      const dropFiles = files.length > 0 ? files : state.dragFiles;
      if (dropFiles.length === 0) {
        console.warn("No files in drop event");
        state.clearDragState();
        return;
      }

      const exts =
        state.dragExtensions.length > 0
          ? state.dragExtensions
          : dropFiles.map((f) => f.split(".").pop()?.toLowerCase() ?? "").filter(Boolean);
      const visible = filterActions(state.actions, state.currentPage, exts);
      const idx = hitTestWedge(x, y, visible.length, 200, 200);
      const wedgeId = idx !== null ? visible[idx]?.id : state.hoveredWedge;

      if (!wedgeId) {
        console.warn("No wedge selected at drop position:", x, y);
        state.clearDragState();
        return;
      }

      console.log("Dispatching dropped action:", wedgeId, dropFiles);
      invoke("dispatch_action", {
        request: {
          action_id: wedgeId,
          files: dropFiles,
          params: {},
        },
      })
        .then((jobId) => {
          console.log("Action dispatched successfully:", jobId);
        })
        .catch((e) => console.error("Dispatch failed:", e));

      state.clearDragState();
    };

    // Drag armed from low-level hook — window positioned and shown
    listen<DragArmedEvent>("drag-armed", ({ payload }) => {
      useWheelStore.getState().setDragState([], [], payload.x, payload.y);
    }).then((u) => unlisteners.push(u));

    // Cursor move from low-level mouse hook
    listen<{ x: number; y: number }>("cursor-move", ({ payload }) => {
      const state = useWheelStore.getState();
      if (!state.isDragging) return;
      const dx = payload.x - state.cursorX;
      const dy = payload.y - state.cursorY;
      const clientX = 200 + dx;
      const clientY = 200 + dy;
      state.setCursor(clientX, clientY);
      const visible = filterActions(state.actions, state.currentPage, state.dragExtensions);
      const idx = hitTestWedge(clientX, clientY, visible.length, 200, 200);
      state.setHoveredWedge(idx !== null ? visible[idx]?.id ?? null : null);
    }).then((u) => unlisteners.push(u));

    // Native Tauri 2 Webview drag drop event listener
    try {
      const appWindow = getCurrentWebviewWindow();
      appWindow
        .onDragDropEvent((event) => {
          const payload = event.payload;
          const dpr = window.devicePixelRatio || 1;

          if (payload.type === "enter") {
            const files = payload.paths ?? [];
            const extensions = files
              .map((f) => f.split(".").pop()?.toLowerCase() ?? "")
              .filter(Boolean);
            const px = payload.position.x / dpr;
            const py = payload.position.y / dpr;
            useWheelStore.getState().setDragState(files, extensions, px, py);
          } else if (payload.type === "over") {
            const px = payload.position.x / dpr;
            const py = payload.position.y / dpr;
            const state = useWheelStore.getState();
            state.setCursor(px, py);
            const visible = filterActions(state.actions, state.currentPage, state.dragExtensions);
            const idx = hitTestWedge(px, py, visible.length, 200, 200);
            state.setHoveredWedge(idx !== null ? visible[idx]?.id ?? null : null);
          } else if (payload.type === "drop") {
            const files = payload.paths ?? [];
            const px = payload.position.x / dpr;
            const py = payload.position.y / dpr;
            handleFilesDropped(files, px, py);
          } else if (payload.type === "leave") {
            useWheelStore.getState().setHoveredWedge(null);
          }
        })
        .then((u) => unlisteners.push(u))
        .catch(console.error);
    } catch (e) {
      console.error("Failed to attach onDragDropEvent:", e);
    }

    // HTML5 dragover & drop fallback for Chromium WebView2
    const handleDragOver = (e: DragEvent) => {
      e.preventDefault();
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = "copy";
      }
      const state = useWheelStore.getState();
      const x = e.clientX;
      const y = e.clientY;
      state.setCursor(x, y);
      const visible = filterActions(state.actions, state.currentPage, state.dragExtensions);
      const idx = hitTestWedge(x, y, visible.length, 200, 200);
      state.setHoveredWedge(idx !== null ? visible[idx]?.id ?? null : null);
    };

    const handleDrop = (e: DragEvent) => {
      e.preventDefault();
      const files: string[] = [];
      if (e.dataTransfer?.files) {
        for (let i = 0; i < e.dataTransfer.files.length; i++) {
          const file = e.dataTransfer.files[i] as any;
          if (file.path) {
            files.push(file.path);
          }
        }
      }
      handleFilesDropped(files, e.clientX, e.clientY);
    };

    window.addEventListener("dragover", handleDragOver);
    window.addEventListener("drop", handleDrop);
    unlisteners.push(() => window.removeEventListener("dragover", handleDragOver));
    unlisteners.push(() => window.removeEventListener("drop", handleDrop));

    // Custom OLE events from Rust IDropTarget fallback
    listen<DropEnterEvent>("drop-enter", ({ payload }) => {
      useWheelStore.getState().setDragState(payload.files, payload.extensions, payload.x, payload.y);
    }).then((u) => unlisteners.push(u));

    listen<{ x: number; y: number }>("drop-over", ({ payload }) => {
      const state = useWheelStore.getState();
      state.setCursor(payload.x, payload.y);
      const visible = filterActions(state.actions, state.currentPage, state.dragExtensions);
      const idx = hitTestWedge(payload.x, payload.y, visible.length, 200, 200);
      state.setHoveredWedge(idx !== null ? visible[idx]?.id ?? null : null);
    }).then((u) => unlisteners.push(u));

    listen("drop-leave", () => {
      useWheelStore.getState().setHoveredWedge(null);
    }).then((u) => unlisteners.push(u));

    listen<DropFilesEvent>("drop-files", ({ payload }) => {
      handleFilesDropped(payload.files, payload.x, payload.y);
    }).then((u) => unlisteners.push(u));

    // Drag cancelled (button released outside or Escape)
    listen("drag-cancelled", () => {
      useWheelStore.getState().clearDragState();
    }).then((u) => unlisteners.push(u));

    // Keyboard: Tab/Space to toggle page while dragging
    const handleKey = (e: KeyboardEvent) => {
      const state = useWheelStore.getState();
      if (state.isDragging) {
        if (e.key === "Tab" || e.key === " ") {
          e.preventDefault();
          state.togglePage();
        } else if (e.key === "Escape") {
          state.clearDragState();
        }
      }
    };
    window.addEventListener("keydown", handleKey);
    unlisteners.push(() => window.removeEventListener("keydown", handleKey));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const handleWedgeDrop = useCallback(
    async (actionId: string, files: string[]) => {
      try {
        await invoke("dispatch_action", {
          request: {
            action_id: actionId,
            files,
            params: {},
          },
        });
      } catch (e) {
        console.error("Dispatch error:", e);
      }
      useWheelStore.getState().clearDragState();
    },
    []
  );

  return (
    <div
      ref={overlayRef}
      className="w-full h-full flex items-center justify-center"
      style={{ background: "transparent" }}
    >
      <AnimatePresence>
        {store.isDragging && (
          <motion.div
            key="wheel"
            initial={{ scale: 0.5, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.4, opacity: 0 }}
            transition={{
              type: "spring",
              stiffness: 400,
              damping: 30,
              duration: 0.15,
            }}
          >
            <RadialWheel
              files={store.dragFiles}
              extensions={store.dragExtensions}
              currentPage={store.currentPage}
              onTogglePage={store.togglePage}
              onWedgeHover={store.setHoveredWedge}
              onWedgeDrop={handleWedgeDrop}
              hoveredWedge={store.hoveredWedge}
            />
          </motion.div>
        )}
      </AnimatePresence>

      {/* Debug overlay in dev */}
      {import.meta.env.DEV && store.isDragging && (
        <div className="absolute bottom-2 left-2 text-xs text-white/50 font-mono">
          {store.dragFiles.length} file(s) | page: {store.currentPage} | wedge: {store.hoveredWedge ?? "none"}
        </div>
      )}
    </div>
  );
}
