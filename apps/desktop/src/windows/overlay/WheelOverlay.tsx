import { useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
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

    // Drag armed — position pre-computed in Rust; overlay already shown
    listen<DragArmedEvent>("drag-armed", ({ payload }) => {
      // The Rust side positioned the window; this event tells the wheel where center is
      store.setDragState([], [], payload.x, payload.y);
    }).then((u) => unlisteners.push(u));

    // OLE DragEnter — files confirmed, set context
    listen<DropEnterEvent>("drop-enter", ({ payload }) => {
      store.setDragState(payload.files, payload.extensions, payload.x, payload.y);
    }).then((u) => unlisteners.push(u));

    // OLE DragOver — update cursor for hit testing
    listen<{ x: number; y: number }>("drop-over", ({ payload }) => {
      store.setCursor(payload.x, payload.y);
      const visible = filterActions(store.actions, store.currentPage, store.dragExtensions);
      const idx = hitTestWedge(payload.x, payload.y, visible.length, 200, 200);
      store.setHoveredWedge(idx !== null ? visible[idx]?.id ?? null : null);
    }).then((u) => unlisteners.push(u));

    // OLE DragLeave — clear (but keep overlay visible — user might re-enter)
    listen("drop-leave", () => {
      // Keep shown but remove hover state
      store.setHoveredWedge(null);
    }).then((u) => unlisteners.push(u));

    // OLE Drop — dispatch the action
    listen<DropFilesEvent>("drop-files", ({ payload }) => {
      const visible = filterActions(store.actions, store.currentPage, store.dragExtensions);
      const idx = hitTestWedge(payload.x, payload.y, visible.length, 200, 200);
      const wedgeId = idx !== null ? visible[idx]?.id : store.hoveredWedge;
      if (!wedgeId) {
        store.clearDragState();
        return;
      }

      invoke("dispatch_action", {
        request: {
          action_id: wedgeId,
          files: payload.files,
          params: {},
        },
      })
        .then((jobId) => {
          console.log("Action dispatched:", jobId);
        })
        .catch((e) => console.error("Dispatch failed:", e));

      store.clearDragState();
    }).then((u) => unlisteners.push(u));

    // Drag cancelled (button released or Escape)
    listen("drag-cancelled", () => {
      store.clearDragState();
    }).then((u) => unlisteners.push(u));

    // Keyboard: Tab/Space to toggle page while dragging
    const handleKey = (e: KeyboardEvent) => {
      if (store.isDragging) {
        if (e.key === "Tab" || e.key === " ") {
          e.preventDefault();
          store.togglePage();
        } else if (e.key === "Escape") {
          store.clearDragState();
        }
      }
    };
    window.addEventListener("keydown", handleKey);

    return () => {
      unlisteners.forEach((u) => u());
      window.removeEventListener("keydown", handleKey);
    };
  }, [store]);

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
      store.clearDragState();
    },
    [store]
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
