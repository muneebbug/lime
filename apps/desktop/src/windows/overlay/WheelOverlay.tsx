import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "motion/react";
import { Loader2, CheckCircle2, AlertCircle } from "lucide-react";
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

interface ToastInfo {
  id: string;
  type: "loading" | "success" | "error";
  message: string;
  subtext?: string;
}

export function WheelOverlay() {
  const isDragging = useWheelStore((s) => s.isDragging);
  const dragFiles = useWheelStore((s) => s.dragFiles);
  const dragExtensions = useWheelStore((s) => s.dragExtensions);
  const currentPage = useWheelStore((s) => s.currentPage);
  const hoveredWedge = useWheelStore((s) => s.hoveredWedge);
  const loadActions = useWheelStore((s) => s.loadActions);
  const togglePage = useWheelStore((s) => s.togglePage);
  const setHoveredWedge = useWheelStore((s) => s.setHoveredWedge);

  const overlayRef = useRef<HTMLDivElement>(null);
  const [toast, setToast] = useState<ToastInfo | null>(null);

  const lastHoveredRef = useRef<string | null>(null);
  const lastDropPosRef = useRef<{ x: number; y: number }>({ x: 200, y: 200 });

  useEffect(() => {
    loadActions();
    useWheelStore.getState().setPage("convert");
  }, [loadActions]);

  const triggerAction = useCallback((actionId: string, files: string[]) => {
    const isConvert = actionId.startsWith("convert.");
    const targetName = isConvert
      ? actionId.replace("convert.", "").toUpperCase()
      : actionId.replace("tool.", "");

    if (isConvert) {
      setToast({
        id: Date.now().toString(),
        type: "loading",
        message: `Converting to ${targetName}...`,
        subtext: files.map((f) => f.split(/[/\\]/).pop()).join(", "),
      });
    }

    lastHoveredRef.current = null;
    useWheelStore.getState().clearDragState();

    invoke("dispatch_action", {
      request: {
        action_id: actionId,
        files,
        params: {},
      },
    })
      .then((jobId) => {
        console.log("Action dispatched successfully:", jobId);
      })
      .catch((e) => {
        console.error("Dispatch failed:", e);
        setToast({
          id: Date.now().toString(),
          type: "error",
          message: "Action failed",
          subtext: String(e),
        });
        setTimeout(() => {
          setToast((cur) => (cur?.type === "error" ? null : cur));
          invoke("hide_overlay");
        }, 5000);
      });
  }, []);

  const handlePointerMove = useCallback((x: number, y: number) => {
    lastDropPosRef.current = { x, y };
    const state = useWheelStore.getState();
    const visible = filterActions(state.actions, state.currentPage, state.dragExtensions);
    const idx = hitTestWedge(x, y, visible.length, 200, 200);
    const newHovered = idx !== null ? visible[idx]?.id ?? null : null;

    if (newHovered !== lastHoveredRef.current) {
      lastHoveredRef.current = newHovered;
      state.setHoveredWedge(newHovered);
    }
  }, []);

  const handlePointerLeave = useCallback(() => {
    if (lastHoveredRef.current !== null) {
      lastHoveredRef.current = null;
      useWheelStore.getState().setHoveredWedge(null);
    }
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

      const dropX = x && x > 0 ? x : lastDropPosRef.current.x;
      const dropY = y && y > 0 ? y : lastDropPosRef.current.y;

      const idx = hitTestWedge(dropX, dropY, visible.length, 200, 200);
      const wedgeId = idx !== null ? visible[idx]?.id : (state.hoveredWedge ?? lastHoveredRef.current);

      lastHoveredRef.current = null;

      if (!wedgeId) {
        console.warn("No wedge selected at drop position:", dropX, dropY);
        state.clearDragState();
        return;
      }

      triggerAction(wedgeId, dropFiles);
    };

    listen<{ job_id: string; outputs: string[] }>("job-completed", ({ payload }) => {
      const outputNames = payload.outputs
        .map((p) => p.split(/[/\\]/).pop())
        .filter(Boolean)
        .join(", ");
      setToast({
        id: Date.now().toString(),
        type: "success",
        message: "Complete",
        subtext: outputNames || "File converted successfully",
      });
      setTimeout(() => {
        setToast((cur) => (cur?.type === "success" ? null : cur));
        invoke("hide_overlay");
      }, 2500);
    }).then((u) => unlisteners.push(u));

    listen<{ job_id: string; error: string }>("job-failed", ({ payload }) => {
      setToast({
        id: Date.now().toString(),
        type: "error",
        message: "Conversion failed",
        subtext: payload.error,
      });
      setTimeout(() => {
        setToast((cur) => (cur?.type === "error" ? null : cur));
        invoke("hide_overlay");
      }, 6000);
    }).then((u) => unlisteners.push(u));

    // Drag armed from low-level hook — window positioned and shown
    listen<DragArmedEvent>("drag-armed", ({ payload }) => {
      lastHoveredRef.current = null;
      lastDropPosRef.current = { x: 200, y: 200 };
      const store = useWheelStore.getState();
      store.setPage("convert");
      store.setDragState([], [], payload.x, payload.y);
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
            lastHoveredRef.current = null;
            lastDropPosRef.current = { x: px, y: py };
            useWheelStore.getState().setDragState(files, extensions, px, py);
          } else if (payload.type === "over") {
            handlePointerMove(payload.position.x / dpr, payload.position.y / dpr);
          } else if (payload.type === "drop") {
            const files = payload.paths ?? [];
            const px = payload.position.x / dpr;
            const py = payload.position.y / dpr;
            handleFilesDropped(files, px, py);
          } else if (payload.type === "leave") {
            handlePointerLeave();
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
      handlePointerMove(e.clientX, e.clientY);
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
      const dpr = window.devicePixelRatio || 1;
      const px = payload.x / dpr;
      const py = payload.y / dpr;
      lastHoveredRef.current = null;
      lastDropPosRef.current = { x: px, y: py };
      useWheelStore.getState().setDragState(payload.files, payload.extensions, px, py);
    }).then((u) => unlisteners.push(u));

    listen<{ x: number; y: number }>("drop-over", ({ payload }) => {
      const dpr = window.devicePixelRatio || 1;
      handlePointerMove(payload.x / dpr, payload.y / dpr);
    }).then((u) => unlisteners.push(u));

    listen("drop-leave", () => {
      handlePointerLeave();
    }).then((u) => unlisteners.push(u));

    listen<DropFilesEvent>("drop-files", ({ payload }) => {
      const dpr = window.devicePixelRatio || 1;
      handleFilesDropped(payload.files, payload.x / dpr, payload.y / dpr);
    }).then((u) => unlisteners.push(u));

    // Drag cancelled (button released outside or Escape)
    listen("drag-cancelled", () => {
      lastHoveredRef.current = null;
      useWheelStore.getState().clearDragState();
    }).then((u) => unlisteners.push(u));

    // Toggle page event from low-level hook (scroll wheel, right-click, Tab, or Space)
    listen("toggle-page", () => {
      useWheelStore.getState().togglePage();
    }).then((u) => unlisteners.push(u));

    // Mouse scroll wheel inside webview to toggle page
    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      useWheelStore.getState().togglePage();
    };
    window.addEventListener("wheel", handleWheel, { passive: false });
    unlisteners.push(() => window.removeEventListener("wheel", handleWheel));

    // Keyboard: Tab/Space to toggle page while dragging
    const handleKey = (e: KeyboardEvent) => {
      const state = useWheelStore.getState();
      if (state.isDragging) {
        if (e.key === "Tab" || e.key === " ") {
          e.preventDefault();
          state.togglePage();
        } else if (e.key === "Escape") {
          lastHoveredRef.current = null;
          state.clearDragState();
        }
      }
    };
    window.addEventListener("keydown", handleKey);
    unlisteners.push(() => window.removeEventListener("keydown", handleKey));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [triggerAction, handlePointerMove, handlePointerLeave]);

  const handleWedgeDrop = useCallback(
    (actionId: string, files: string[]) => {
      triggerAction(actionId, files);
    },
    [triggerAction]
  );

  return (
    <div
      ref={overlayRef}
      className="w-full h-full flex items-center justify-center relative select-none"
      style={{ background: "transparent" }}
    >
      <AnimatePresence>
        {isDragging && (
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
              files={dragFiles}
              extensions={dragExtensions}
              currentPage={currentPage}
              onTogglePage={togglePage}
              onWedgeHover={setHoveredWedge}
              onWedgeDrop={handleWedgeDrop}
              hoveredWedge={hoveredWedge}
            />
          </motion.div>
        )}
      </AnimatePresence>

      {/* Floating HUD toast for status, progress, and errors */}
      <AnimatePresence>
        {toast && (
          <motion.div
            key={toast.id}
            initial={{ scale: 0.85, opacity: 0, y: 8 }}
            animate={{ scale: 1, opacity: 1, y: 0 }}
            exit={{ scale: 0.85, opacity: 0, y: 8 }}
            transition={{ type: "spring", stiffness: 450, damping: 28 }}
            className="absolute z-50 flex flex-col items-center justify-center max-w-[340px] px-5 py-4 rounded-2xl bg-neutral-950/92 border border-white/15 backdrop-blur-2xl shadow-2xl text-center cursor-pointer pointer-events-auto"
            onClick={() => {
              setToast(null);
              invoke("hide_overlay");
            }}
          >
            {toast.type === "loading" && (
              <Loader2 className="w-6 h-6 text-sky-400 animate-spin mb-2" />
            )}
            {toast.type === "success" && (
              <CheckCircle2 className="w-6 h-6 text-emerald-400 mb-2" />
            )}
            {toast.type === "error" && (
              <AlertCircle className="w-6 h-6 text-rose-400 mb-2" />
            )}
            <div className="text-sm font-semibold text-white tracking-wide">
              {toast.message}
            </div>
            {toast.subtext && (
              <div className="text-xs text-neutral-300 mt-1 line-clamp-4 leading-relaxed font-sans">
                {toast.subtext}
              </div>
            )}
            {toast.type === "error" && (
              <div className="text-[10px] text-neutral-400 mt-2 font-mono uppercase tracking-wider">
                Click to dismiss
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>

    </div>
  );
}
