import React, { useEffect } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { RotateCcw, Loader2, FolderOpen, ExternalLink } from "lucide-react";
import { CaptionButtons, WheelButton } from "../../ui/WheelUI";

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
    appWindow.unminimize().catch(() => {});
    appWindow.show().catch(() => {});
    appWindow.setFocus().catch(() => {});
  }, [appWindow]);

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
    <div className="flex flex-col h-screen w-screen bg-[#151516] text-neutral-200 font-sans select-none overflow-hidden rounded-xl border border-white/[0.08] shadow-2xl">
      {/* Window Titlebar Header */}
      <header
        data-tauri-drag-region
        onMouseDown={(e) => {
          if (e.button === 0 && !(e.target as HTMLElement).closest("button, input, select, textarea, [data-no-drag], [data-tauri-drag-region='false']")) {
            appWindow.startDragging().catch(() => {});
          }
        }}
        className="h-10 flex items-center justify-between pl-3 pr-0 border-b border-white/[0.06] bg-[#151516] select-none shrink-0 z-20"
      >
        {/* Left: Tool identity & filename badge */}
        <div data-tauri-drag-region className="flex items-center gap-2.5 min-w-0 pointer-events-none">
          {icon && (
            <div className="w-5 h-5 rounded-md bg-white/[0.08] border border-white/[0.06] flex items-center justify-center text-neutral-300 shrink-0">
              {icon}
            </div>
          )}
          <span className="text-[13px] font-medium text-neutral-200 truncate">
            {title}
          </span>
          {subtitle && (
            <span className="text-[11px] font-mono text-neutral-400 bg-white/[0.04] px-2 py-0.5 rounded border border-white/[0.06] truncate max-w-xs">
              {subtitle}
            </span>
          )}
        </div>

        {/* Right: Keyboard shortcut badge and Windows caption buttons */}
        <div className="flex items-center gap-3 h-full">
          <span className="hidden sm:inline-block px-1.5 py-0.5 bg-white/[0.04] border border-white/[0.06] text-[10px] text-neutral-400 font-mono rounded pointer-events-none">
            Esc to cancel
          </span>
          <CaptionButtons />
        </div>
      </header>

      {/* Main Tool Content Workspace */}
      <main className="flex-1 bg-[#19191a] flex flex-col overflow-hidden relative">
        {children}
      </main>

      {/* Action Bar Footer */}
      <footer className="h-12 px-4 bg-[#151516] border-t border-white/[0.06] flex items-center justify-between shrink-0 select-none z-20">
        {/* Left: Secondary actions or success state */}
        <div className="flex items-center gap-2">
          {successResultPath ? (
            <div className="flex items-center gap-2 text-xs text-emerald-400 font-medium">
              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              <span>Saved successfully</span>
              <button
                onClick={handleOpenFolder}
                className="ml-2 flex items-center gap-1 px-2.5 py-1 rounded-md bg-white/[0.06] hover:bg-white/[0.1] text-neutral-200 border border-white/[0.08] transition-colors cursor-default text-xs"
              >
                <FolderOpen size={12} />
                <span>Show in folder</span>
              </button>
              <button
                onClick={handleOpenFile}
                className="flex items-center gap-1 px-2.5 py-1 rounded-md bg-white/[0.06] hover:bg-white/[0.1] text-neutral-200 border border-white/[0.08] transition-colors cursor-default text-xs"
              >
                <ExternalLink size={12} />
                <span>Open</span>
              </button>
            </div>
          ) : (
            <WheelButton
              variant="secondary"
              onClick={onReset}
              disabled={isProcessing}
            >
              <RotateCcw size={12} className="inline mr-1 text-neutral-400" />
              Reset
            </WheelButton>
          )}
        </div>

        {/* Right: Primary Action Button with Enter kbd hint */}
        <div className="flex items-center gap-2">
          <WheelButton
            variant="primary"
            onClick={onPrimaryAction}
            disabled={isProcessing}
            kbd="↵"
          >
            {isProcessing ? (
              <span className="flex items-center gap-1.5">
                <Loader2 size={13} className="animate-spin" />
                Processing...
              </span>
            ) : (
              primaryActionLabel
            )}
          </WheelButton>
        </div>
      </footer>
    </div>
  );
}
