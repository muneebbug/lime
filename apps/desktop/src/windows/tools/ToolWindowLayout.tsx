import React, { useEffect } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { X, RotateCcw, Loader2, FolderOpen, ExternalLink } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";

export interface ToolWindowLayoutProps {
  title: string;
  subtitle?: string;
  icon?: React.ReactNode;
  children: React.ReactNode;
  primaryActionLabel: string;
  isProcessing?: boolean;
  successResultPath?: string | null;
  onReset: () => void;
  onPrimaryAction: () => void;
}

export function ToolWindowLayout({
  title,
  subtitle,
  icon,
  children,
  primaryActionLabel,
  isProcessing = false,
  successResultPath = null,
  onReset,
  onPrimaryAction,
}: ToolWindowLayoutProps) {
  const appWindow = getCurrentWebviewWindow();

  const handleClose = async () => {
    try {
      await appWindow.close();
    } catch (e) {
      console.error("Failed to close window", e);
    }
  };

  const handleOpenFolder = async () => {
    if (successResultPath) {
      try {
        await invoke("open_in_folder", { path: successResultPath });
      } catch (e) {
        console.error("Failed to open folder", e);
      }
    }
  };

  const handleOpenFile = async () => {
    if (successResultPath) {
      try {
        await invoke("open_file", { path: successResultPath });
      } catch (e) {
        console.error("Failed to open file", e);
      }
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        handleClose();
      } else if (e.key === "Enter" && !e.shiftKey) {
        // Trigger primary action if not inside a textarea
        if ((e.target as HTMLElement)?.tagName !== "TEXTAREA") {
          e.preventDefault();
          onPrimaryAction();
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onPrimaryAction]);

  return (
    <div className="flex flex-col h-screen w-screen bg-zinc-950/90 text-zinc-100 font-sans select-none overflow-hidden rounded-2xl border border-white/10 shadow-2xl backdrop-blur-2xl">
      {/* Background warm gradient accent */}
      <div className="absolute inset-0 pointer-events-none bg-[radial-gradient(ellipse_at_50%_-10%,rgba(249,115,22,0.18),transparent_65%)]" />

      {/* Header: Draggable chrome */}
      <header
        data-tauri-drag-region
        className="relative z-10 flex items-center justify-between px-4 py-3 border-b border-white/10 bg-zinc-900/40 select-none cursor-move"
      >
        {/* Left: Circular close button */}
        <div className="flex items-center gap-2">
          <button
            onClick={handleClose}
            className="w-7 h-7 rounded-full flex items-center justify-center bg-white/5 hover:bg-red-500/20 hover:text-red-400 border border-white/10 transition-colors text-zinc-400 cursor-pointer"
            title="Close (Esc)"
          >
            <X size={14} strokeWidth={2.5} />
          </button>
        </div>

        {/* Center: Title & subtitle */}
        <div className="flex flex-col items-center pointer-events-none">
          <div className="flex items-center gap-2">
            {icon && <span className="text-orange-400">{icon}</span>}
            <span className="text-sm font-semibold tracking-wide text-zinc-200">
              {title}
            </span>
          </div>
          {subtitle && (
            <span className="text-[11px] text-zinc-400 font-mono truncate max-w-[280px]">
              {subtitle}
            </span>
          )}
        </div>

        {/* Right balance spacer */}
        <div className="w-7" />
      </header>

      {/* Main tool body */}
      <main className="relative z-10 flex-1 overflow-auto p-4 flex flex-col">
        {children}
      </main>

      {/* Footer */}
      <footer className="relative z-10 flex items-center justify-between px-5 py-3 border-t border-white/10 bg-zinc-900/60 backdrop-blur-md">
        {/* Reset button */}
        <button
          onClick={onReset}
          disabled={isProcessing}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-zinc-400 hover:text-zinc-200 hover:bg-white/5 transition-colors cursor-pointer disabled:opacity-40"
        >
          <RotateCcw size={13} />
          <span>Reset</span>
        </button>

        {/* Primary action or success actions */}
        <div className="flex items-center gap-2">
          <AnimatePresence mode="wait">
            {successResultPath ? (
              <motion.div
                key="success"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95 }}
                className="flex items-center gap-2"
              >
                <button
                  onClick={handleOpenFile}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-white/10 transition-colors cursor-pointer"
                >
                  <ExternalLink size={13} />
                  <span>Open</span>
                </button>
                <button
                  onClick={handleOpenFolder}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-orange-600 hover:bg-orange-500 text-white font-semibold shadow-lg shadow-orange-950/50 transition-all cursor-pointer"
                >
                  <FolderOpen size={13} />
                  <span>Show in Folder</span>
                </button>
              </motion.div>
            ) : (
              <motion.button
                key="action"
                onClick={onPrimaryAction}
                disabled={isProcessing}
                whileTap={{ scale: 0.97 }}
                className="flex items-center gap-2 px-5 py-1.5 rounded-lg text-xs font-semibold bg-gradient-to-r from-orange-500 to-amber-500 hover:from-orange-400 hover:to-amber-400 text-zinc-950 shadow-lg shadow-orange-950/60 transition-all cursor-pointer disabled:opacity-50"
              >
                {isProcessing ? (
                  <>
                    <Loader2 size={14} className="animate-spin text-zinc-950" />
                    <span>Processing...</span>
                  </>
                ) : (
                  <span>{primaryActionLabel}</span>
                )}
              </motion.button>
            )}
          </AnimatePresence>
        </div>
      </footer>
    </div>
  );
}
