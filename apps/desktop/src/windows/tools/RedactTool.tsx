import React, { useState, useRef, useEffect, useCallback } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import {
  ShieldAlert,
  Undo2,
  Trash2,
  Square,
  Grid3X3,
  EyeOff,
  AlertTriangle,
  Sparkles,
} from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";

export interface RedactToolProps {
  filePath: string;
}

export type RedactMode = "black_bar" | "pixelate" | "blur";

export interface RedactRegion {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  mode: RedactMode;
}

export function RedactTool({ filePath }: RedactToolProps) {
  const [naturalSize, setNaturalSize] = useState<{ width: number; height: number } | null>(null);
  const [activeMode, setActiveMode] = useState<RedactMode>("black_bar");
  const [regions, setRegions] = useState<RedactRegion[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Active drag state for creating a new region
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(null);
  const [currentBox, setCurrentBox] = useState<{ x: number; y: number; width: number; height: number } | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);

  const assetUrl = convertFileSrc(filePath);
  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  const handleImageLoad = (e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    setNaturalSize({ width: img.naturalWidth, height: img.naturalHeight });
  };

  // Convert client viewport coordinates to natural image pixel coordinates
  const clientToImageCoords = useCallback(
    (clientX: number, clientY: number) => {
      const img = imgRef.current;
      if (!img || !naturalSize) return null;

      const rect = img.getBoundingClientRect();
      const clickX = clientX - rect.left;
      const clickY = clientY - rect.top;

      // Clamp inside image bounds
      const clampedX = Math.max(0, Math.min(clickX, rect.width));
      const clampedY = Math.max(0, Math.min(clickY, rect.height));

      const scaleX = naturalSize.width / rect.width;
      const scaleY = naturalSize.height / rect.height;

      return {
        x: Math.round(clampedX * scaleX),
        y: Math.round(clampedY * scaleY),
        screenX: clampedX,
        screenY: clampedY,
        imgWidth: rect.width,
        imgHeight: rect.height,
      };
    },
    [naturalSize]
  );

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0 || !naturalSize) return;
    const pt = clientToImageCoords(e.clientX, e.clientY);
    if (!pt) return;

    setDragStart({ x: pt.x, y: pt.y });
    setCurrentBox({ x: pt.x, y: pt.y, width: 0, height: 0 });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!dragStart || !naturalSize) return;
    const pt = clientToImageCoords(e.clientX, e.clientY);
    if (!pt) return;

    const minX = Math.min(dragStart.x, pt.x);
    const minY = Math.min(dragStart.y, pt.y);
    const width = Math.abs(pt.x - dragStart.x);
    const height = Math.abs(pt.y - dragStart.y);

    setCurrentBox({ x: minX, y: minY, width, height });
  };

  const handleMouseUp = () => {
    if (currentBox && currentBox.width > 5 && currentBox.height > 5) {
      const newRegion: RedactRegion = {
        id: Math.random().toString(36).substring(2, 9),
        x: Math.round(currentBox.x),
        y: Math.round(currentBox.y),
        width: Math.round(currentBox.width),
        height: Math.round(currentBox.height),
        mode: activeMode,
      };
      setRegions((prev) => [...prev, newRegion]);
    }
    setDragStart(null);
    setCurrentBox(null);
  };

  // Keyboard shortcut Ctrl+Z to undo
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "z") {
        e.preventDefault();
        setRegions((prev) => prev.slice(0, -1));
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const handleUndo = () => {
    setRegions((prev) => prev.slice(0, -1));
  };

  const handleClear = () => {
    setRegions([]);
  };

  const handleDeleteRegion = (id: string) => {
    setRegions((prev) => prev.filter((r) => r.id !== id));
  };

  const handlePrimaryAction = async () => {
    if (regions.length === 0) {
      setErrorMessage("Please select at least one region to redact.");
      return;
    }

    setIsProcessing(true);
    setErrorMessage(null);
    try {
      const backendRegions = regions.map((r) => ({
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
        mode: r.mode,
      }));

      const out = await invoke<string>("redact_image_file", {
        inputPath: filePath,
        regions: backendRegions,
        outputPath: null,
      });

      setSuccessPath(out);
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err.message ?? "Redaction failed");
    } finally {
      setIsProcessing(false);
    }
  };

  // Helper to calculate CSS relative bounding box percentage on the rendered image
  const getStyleForRegion = (r: { x: number; y: number; width: number; height: number }) => {
    if (!naturalSize) return {};
    const leftPct = (r.x / naturalSize.width) * 100;
    const topPct = (r.y / naturalSize.height) * 100;
    const widthPct = (r.width / naturalSize.width) * 100;
    const heightPct = (r.height / naturalSize.height) * 100;

    return {
      left: `${leftPct}%`,
      top: `${topPct}%`,
      width: `${widthPct}%`,
      height: `${heightPct}%`,
    };
  };

  return (
    <ToolWindowLayout
      title="Redact"
      subtitle={fileName}
      icon={<ShieldAlert size={16} className="text-orange-500" />}
      primaryActionLabel={regions.length > 0 ? `Burn & Save (${regions.length})` : "Redact"}
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleClear}
      onPrimaryAction={handlePrimaryAction}
    >
      <div className="flex flex-col h-full w-full overflow-hidden">
        {/* Top Control Bar: Mode selection & Warning */}
        <div className="flex flex-wrap items-center justify-between gap-3 px-5 py-2.5 bg-[#181818] border-b border-white/[0.06]">
          {/* Modes */}
          <div className="flex items-center gap-0.5 p-0.5 bg-[#242424] rounded-lg border border-white/[0.08]">
            <button
              onClick={() => setActiveMode("black_bar")}
              className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeMode === "black_bar"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
            >
              <Square size={12} className="fill-current" />
              <span>Black Bar</span>
            </button>

            <button
              onClick={() => setActiveMode("pixelate")}
              className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeMode === "pixelate"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
            >
              <Grid3X3 size={12} />
              <span>Pixelate</span>
            </button>

            <button
              onClick={() => setActiveMode("blur")}
              className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeMode === "blur"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
            >
              <EyeOff size={12} />
              <span>Heavy Blur</span>
            </button>
          </div>

          {/* Quick Actions: Undo & Clear */}
          <div className="flex items-center gap-2">
            <button
              onClick={handleUndo}
              disabled={regions.length === 0}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium text-neutral-300 hover:text-white bg-[#242424] hover:bg-[#2c2c2c] border border-white/[0.08] disabled:opacity-40 disabled:pointer-events-none transition-colors cursor-default"
              title="Undo last box (Ctrl+Z)"
            >
              <Undo2 size={12} />
              <span>Undo</span>
            </button>

            <button
              onClick={handleClear}
              disabled={regions.length === 0}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium text-neutral-400 hover:text-red-400 bg-[#242424] hover:bg-red-500/10 border border-white/[0.08] disabled:opacity-40 disabled:pointer-events-none transition-colors cursor-default"
              title="Clear all redactions"
            >
              <Trash2 size={12} />
              <span>Clear</span>
            </button>
          </div>
        </div>

        {/* Warning Banner */}
        <div className="flex items-center gap-2 px-5 py-2 bg-amber-500/10 border-b border-amber-500/20 text-amber-300 text-xs">
          <AlertTriangle size={13} className="shrink-0 text-amber-400" />
          <span>
            <strong>Irreversible pixel burn</strong>: Redactions permanently overwrite pixel data. All EXIF & location metadata will be stripped.
          </span>
        </div>

        {/* Main Canvas Area */}
        <div
          ref={containerRef}
          className="relative flex-1 flex items-center justify-center p-6 select-none overflow-hidden bg-[#141414]"
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          style={{ cursor: "crosshair" }}
        >
          <div className="relative inline-block max-h-full max-w-full">
            <img
              ref={imgRef}
              src={assetUrl}
              alt="To redact"
              onLoad={handleImageLoad}
              className="max-h-[58vh] max-w-[80vw] object-contain rounded shadow-2xl pointer-events-none"
              draggable={false}
            />

            {/* Committed Redaction Regions */}
            {naturalSize &&
              regions.map((region) => {
                const style = getStyleForRegion(region);
                return (
                  <div
                    key={region.id}
                    style={style}
                    className="absolute group transition-opacity"
                  >
                    {region.mode === "black_bar" && (
                      <div className="w-full h-full bg-black border border-white/20 shadow-md" />
                    )}

                    {region.mode === "pixelate" && (
                      <div
                        className="w-full h-full border border-orange-400/40 backdrop-blur-md"
                        style={{
                          backgroundImage:
                            "repeating-linear-gradient(0deg, #111, #111 6px, #333 6px, #333 12px), repeating-linear-gradient(90deg, #111, #111 6px, #444 6px, #444 12px)",
                          backgroundBlendMode: "difference",
                        }}
                      />
                    )}

                    {region.mode === "blur" && (
                      <div className="w-full h-full border border-orange-400/40 backdrop-blur-xl bg-white/10" />
                    )}

                    {/* Delete hover badge */}
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteRegion(region.id);
                      }}
                      className="absolute -top-2 -right-2 w-5 h-5 rounded-full bg-red-600 hover:bg-red-500 text-white flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity shadow-lg cursor-default text-xs"
                      title="Delete region"
                    >
                      ×
                    </button>
                  </div>
                );
              })}

            {/* Active Box Being Dragged */}
            {naturalSize && currentBox && currentBox.width > 2 && currentBox.height > 2 && (
              <div
                style={getStyleForRegion(currentBox)}
                className="absolute border-2 border-orange-500 bg-orange-500/20 backdrop-blur-sm pointer-events-none shadow-lg animate-pulse"
              />
            )}
          </div>
        </div>

        {/* Footer info: natural dimensions and count */}
        <div className="flex items-center justify-between px-6 py-2.5 bg-zinc-900/60 border-t border-white/5 text-xs text-zinc-400">
          <div className="flex items-center gap-3">
            <span>
              Regions: <strong className="text-zinc-200">{regions.length}</strong>
            </span>
            {naturalSize && (
              <span>
                Original: <strong className="text-zinc-200">{naturalSize.width} × {naturalSize.height} px</strong>
              </span>
            )}
          </div>

          {errorMessage && (
            <span className="text-red-400 text-xs font-medium">{errorMessage}</span>
          )}

          <div className="flex items-center gap-1.5 text-zinc-500">
            <Sparkles size={12} className="text-orange-400/70" />
            <span>Click and drag on the image to mask confidential data</span>
          </div>
        </div>
      </div>
    </ToolWindowLayout>
  );
}
