import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "motion/react";
import { useWheelStore } from "../../store/wheelStore";
import { playHoverSound } from "../../utils/sound";
import {
  RadialWheel,
  hitTestWedge,
  filterActions,
  hasToolsForExtensions,
  RASTER_IMAGE_EXTS,
  wheelDiameter,
  MEDIA_EXTS,
} from "./RadialWheel";

export function isSupportedFileType(extensions: string[]): boolean {
  if (!extensions || extensions.length === 0) return false;
  return extensions.some((ext) => {
    const clean = ext.toLowerCase().trim();
    return (
      RASTER_IMAGE_EXTS.has(clean) ||
      MEDIA_EXTS.has(clean)
    );
  });
}

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

/**
 * Force a repaint once the stylesheet is present.
 *
 * The wheel reads its colours from CSS custom properties, so the very first
 * paint would otherwise draw with empty colour values if React mounted before
 * the stylesheet resolved.
 */
function useThemeColors() {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    // Reading `cssRules` forces the stylesheet to resolve; then one frame so the
    // SVG paints with real values rather than empty ones.
    try {
      for (const sheet of Array.from(document.styleSheets)) {
        void sheet.cssRules.length;
      }
    } catch {
      // A cross-origin sheet would throw; the tokens are local, so this is fine.
    }
    const frame = requestAnimationFrame(() => setReady(true));
    return () => cancelAnimationFrame(frame);
  }, []);
  return ready;
}

export function WheelOverlay() {
  useThemeColors();
  const isDragging = useWheelStore((s) => s.isDragging);
  const dragFiles = useWheelStore((s) => s.dragFiles);
  const dragExtensions = useWheelStore((s) => s.dragExtensions);
  const currentPage = useWheelStore((s) => s.currentPage);
  const hoveredWedge = useWheelStore((s) => s.hoveredWedge);
  const loadActions = useWheelStore((s) => s.loadActions);
  const togglePage = useWheelStore((s) => s.togglePage);
  const setHoveredWedge = useWheelStore((s) => s.setHoveredWedge);

const overlayRef = useRef<HTMLDivElement>(null);
  const [wheelSettings, setWheelSettings] = useState<any>(null);

  const lastHoveredRef = useRef<string | null>(null);
  const lastDropPosRef = useRef<{ x: number; y: number }>({ x: 200, y: 200 });
  const isDroppingRef = useRef(false);
  const armTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const wheelSize = wheelDiameter(wheelSettings?.wheel_ui?.size);
  const contextFilterEnabled = wheelSettings?.wheel_ui?.context_filter_enabled ?? true;
  const soundEnabled = wheelSettings?.wheel_ui?.sound_enabled ?? false;
  const reducedMotion = wheelSettings?.wheel_ui?.reduced_motion ?? false;

  const hasTools = hasToolsForExtensions(dragExtensions, contextFilterEnabled);

  useEffect(() => {
    if (!hasTools && currentPage === "tools") {
      useWheelStore.getState().setPage("convert");
    }
  }, [hasTools, currentPage]);

  const soundEnabledRef = useRef(soundEnabled);
  useEffect(() => {
    soundEnabledRef.current = soundEnabled;
  }, [soundEnabled]);

  useEffect(() => {
    loadActions();
    invoke<any>("get_settings")
      .then((s) => {
        setWheelSettings(s);
        if (s?.wheel_ui?.sound_enabled !== undefined) {
          soundEnabledRef.current = s.wheel_ui.sound_enabled;
        }
      })
      .catch(console.error);

    const unlisten = listen("settings-updated", (event: any) => {
      const s = event.payload;
      setWheelSettings(s);
      if (s?.wheel_ui?.sound_enabled !== undefined) {
        soundEnabledRef.current = s.wheel_ui.sound_enabled;
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadActions]);

  const triggerAction = useCallback((actionId: string, files: string[]) => {
    lastHoveredRef.current = null;
    useWheelStore.getState().clearDragState();
    useWheelStore.getState().setPage("convert");

    // Progress and results are surfaced by the detached status HUD, which the
    // Rust side shows in response to the dispatch itself.
    invoke("dispatch_action", {
      request: {
        action_id: actionId,
        files,
        params: {},
      },
    }).catch((e) => {
      console.error("Dispatch failed:", e);
    });
  }, []);

  const slotCount = wheelSettings?.wheel_ui?.slot_count ?? 8;

  const handlePointerMove = useCallback((x: number, y: number) => {
    lastDropPosRef.current = { x, y };
    const state = useWheelStore.getState();
    const currentSlotCount = wheelSettings?.wheel_ui?.slot_count ?? 8;
    const visible = filterActions(state.actions, state.currentPage, state.dragExtensions, contextFilterEnabled, currentSlotCount);
    const scale = wheelSize / 272;
    const idx = hitTestWedge(x, y, visible.length, 200, 200, scale);
    const candidate = idx !== null ? visible[idx] : null;
    const newHovered = candidate ? candidate.id : null;

    if (newHovered !== lastHoveredRef.current) {
      const prev = lastHoveredRef.current;
      lastHoveredRef.current = newHovered;
      state.setHoveredWedge(newHovered);
      if (soundEnabledRef.current && newHovered !== null && newHovered !== prev) {
        playHoverSound();
      }
    }
  }, [contextFilterEnabled, wheelSize, wheelSettings]);

  const handlePointerLeave = useCallback(() => {
    if (lastHoveredRef.current !== null) {
      lastHoveredRef.current = null;
      useWheelStore.getState().setHoveredWedge(null);
    }
  }, []);

  const handleFilesEntered = useCallback((files: string[], extensions: string[], x: number, y: number) => {
    if (armTimeoutRef.current) {
      clearTimeout(armTimeoutRef.current);
      armTimeoutRef.current = null;
    }

    if (!files || files.length === 0) {
      useWheelStore.getState().clearDragState();
      invoke("hide_overlay");
      return;
    }

    // Context filter: verify the file type has at least one supported action in Lime
    if (!isSupportedFileType(extensions)) {
      console.log("Lime: ignoring unsupported file type:", extensions);
      useWheelStore.getState().clearDragState();
      invoke("hide_overlay");
      return;
    }

    const hasToolsForFiles = hasToolsForExtensions(extensions, contextFilterEnabled);
    if (!hasToolsForFiles && useWheelStore.getState().currentPage === "tools") {
      useWheelStore.getState().setPage("convert");
    }

    lastHoveredRef.current = null;
    lastDropPosRef.current = { x, y };
    useWheelStore.getState().setDragState(files, extensions, x, y);
  }, [contextFilterEnabled]);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    const handleFilesDropped = (files: string[], x: number, y: number) => {
      if (isDroppingRef.current) {
        return;
      }
      isDroppingRef.current = true;
      setTimeout(() => {
        isDroppingRef.current = false;
      }, 800);

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
      const currentSlotCount = wheelSettings?.wheel_ui?.slot_count ?? 8;
      const visible = filterActions(state.actions, state.currentPage, exts, contextFilterEnabled, currentSlotCount);

      const dropX = x && x > 0 ? x : lastDropPosRef.current.x;
      const dropY = y && y > 0 ? y : lastDropPosRef.current.y;

      const scale = wheelSize / 272;
      const idx = hitTestWedge(dropX, dropY, visible.length, 200, 200, scale);
      const candidate = idx !== null ? visible[idx] : null;

      const wedgeId = candidate ? candidate.id : (state.hoveredWedge ?? lastHoveredRef.current);

      lastHoveredRef.current = null;

      if (!wedgeId) {
        console.warn("No wedge selected at drop position:", dropX, dropY);
        state.clearDragState();
        return;
      }

      triggerAction(wedgeId, dropFiles);
    };

    // Drag armed from low-level hook — window positioned and shown, but wait for drop-enter
    listen<DragArmedEvent>("drag-armed", () => {
      lastHoveredRef.current = null;
      lastDropPosRef.current = { x: 200, y: 200 };
      // Do NOT set isDragging = true here. Keep overlay transparent until files actually enter.
      useWheelStore.getState().clearDragState();

      if (armTimeoutRef.current) {
        clearTimeout(armTimeoutRef.current);
      }
      armTimeoutRef.current = setTimeout(() => {
        const state = useWheelStore.getState();
        if (!state.isDragging) {
          invoke("hide_overlay");
        }
      }, 400);
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
            if (files.length > 0) {
              const extensions = files
                .map((f) => f.split(".").pop()?.toLowerCase() ?? "")
                .filter(Boolean);
              const px = payload.position.x / dpr;
              const py = payload.position.y / dpr;
              handleFilesEntered(files, extensions, px, py);
            }
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

    // Custom OLE events from Rust IDropTarget fallback
    listen<DropEnterEvent>("drop-enter", ({ payload }) => {
      const dpr = window.devicePixelRatio || 1;
      const px = payload.x / dpr;
      const py = payload.y / dpr;
      handleFilesEntered(payload.files, payload.extensions, px, py);
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
      if (armTimeoutRef.current) {
        clearTimeout(armTimeoutRef.current);
        armTimeoutRef.current = null;
      }
      lastHoveredRef.current = null;
      useWheelStore.getState().clearDragState();
    }).then((u) => unlisteners.push(u));

    // Toggle page event from low-level hook (scroll wheel, right-click, Tab, or Space)
    const onToggle = () => {
      const state = useWheelStore.getState();
      const canToggle = hasToolsForExtensions(state.dragExtensions, contextFilterEnabled);
      if (!canToggle) return;
      lastHoveredRef.current = null;
      state.togglePage();
    };

    listen("toggle-page", onToggle).then((u) => unlisteners.push(u));
    try {
      const appWindow = getCurrentWebviewWindow();
      appWindow.listen("toggle-page", onToggle).then((u) => unlisteners.push(u));
    } catch (e) {
      console.error("Failed to attach appWindow toggle-page listener:", e);
    }

    // Mouse scroll wheel inside webview to toggle page
    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onToggle();
    };
    window.addEventListener("wheel", handleWheel, { passive: false });
    unlisteners.push(() => window.removeEventListener("wheel", handleWheel));

    // Keyboard: Tab/Space to toggle page while dragging
    const handleKey = (e: KeyboardEvent) => {
      const state = useWheelStore.getState();
      if (state.isDragging) {
        if (e.key === "Tab" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          onToggle();
        } else if (e.key === "Escape") {
          lastHoveredRef.current = null;
          state.clearDragState();
        }
      }
    };
    window.addEventListener("keydown", handleKey);
    unlisteners.push(() => window.removeEventListener("keydown", handleKey));

    return () => {
      if (armTimeoutRef.current) {
        clearTimeout(armTimeoutRef.current);
        armTimeoutRef.current = null;
      }
      unlisteners.forEach((u) => u());
    };
  }, [triggerAction, handlePointerMove, handlePointerLeave, contextFilterEnabled]);

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
        {isDragging && dragFiles.length > 0 && isSupportedFileType(dragExtensions) && (
          <motion.div
            key="wheel"
            initial={reducedMotion ? { opacity: 0 } : { scale: 0.5, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={reducedMotion ? { opacity: 0 } : { scale: 0.4, opacity: 0 }}
            transition={
              reducedMotion
                ? { duration: 0.05 }
                : {
                    type: "spring",
                    stiffness: 400,
                    damping: 30,
                    duration: 0.15,
                  }
            }
          >
            <RadialWheel
              files={dragFiles}
              extensions={dragExtensions}
              currentPage={currentPage}
              onTogglePage={() => {
                if (hasTools) togglePage();
              }}
              onWedgeHover={setHoveredWedge}
              onWedgeDrop={handleWedgeDrop}
              hoveredWedge={hoveredWedge}
              size={wheelSize}
              contextFilterEnabled={contextFilterEnabled}
              soundEnabled={soundEnabled}
              slotCount={slotCount}
              hasTools={hasTools}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
