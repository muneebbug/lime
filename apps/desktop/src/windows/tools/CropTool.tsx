import { useState, useEffect, useRef } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Crop, Lock, Unlock } from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";
import { RaycastSegmented } from "../../ui/RaycastUI";

export interface CropToolProps {
  filePath: string;
}

type AspectRatio = "free" | "1:1" | "16:9" | "9:16" | "4:3" | "3:4";

interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function CropTool({ filePath }: CropToolProps) {
  const [naturalSize, setNaturalSize] = useState<{ width: number; height: number } | null>(null);
  const [crop, setCrop] = useState<CropRect>({ x: 0, y: 0, width: 100, height: 100 });
  const [aspectRatio, setAspectRatio] = useState<AspectRatio>("free");
  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const dragRef = useRef<{
    mode: "move" | "nw" | "ne" | "sw" | "se" | "n" | "s" | "e" | "w";
    startX: number;
    startY: number;
    startCrop: CropRect;
  } | null>(null);

  const assetUrl = convertFileSrc(filePath);
  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  // On image load, initialize crop to full image
  const handleImageLoad = (e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    const nw = img.naturalWidth;
    const nh = img.naturalHeight;
    setNaturalSize({ width: nw, height: nh });
    setCrop({
      x: Math.round(nw * 0.1),
      y: Math.round(nh * 0.1),
      width: Math.round(nw * 0.8),
      height: Math.round(nh * 0.8),
    });
  };

  // Adjust crop when aspect ratio preset changes
  const applyAspectRatio = (ratio: AspectRatio) => {
    setAspectRatio(ratio);
    if (!naturalSize || ratio === "free") return;

    let targetRatio = 1;
    switch (ratio) {
      case "1:1":
        targetRatio = 1;
        break;
      case "16:9":
        targetRatio = 16 / 9;
        break;
      case "9:16":
        targetRatio = 9 / 16;
        break;
      case "4:3":
        targetRatio = 4 / 3;
        break;
      case "3:4":
        targetRatio = 3 / 4;
        break;
    }

    setCrop((prev) => {
      let newW = prev.width;
      let newH = Math.round(newW / targetRatio);

      if (newH > naturalSize.height) {
        newH = naturalSize.height;
        newW = Math.round(newH * targetRatio);
      }
      if (newW > naturalSize.width) {
        newW = naturalSize.width;
        newH = Math.round(newW / targetRatio);
      }

      const newX = Math.min(prev.x, naturalSize.width - newW);
      const newY = Math.min(prev.y, naturalSize.height - newH);

      return { x: Math.max(0, newX), y: Math.max(0, newY), width: newW, height: newH };
    });
  };

  // Convert natural coordinates to visual container percent (0 to 100%)
  const toPercent = (val: number, max: number) => (max > 0 ? (val / max) * 100 : 0);

  // Drag handles logic
  const handleMouseDown = (
    mode: "move" | "nw" | "ne" | "sw" | "se" | "n" | "s" | "e" | "w",
    e: React.MouseEvent
  ) => {
    e.preventDefault();
    e.stopPropagation();
    dragRef.current = {
      mode,
      startX: e.clientX,
      startY: e.clientY,
      startCrop: { ...crop },
    };
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!dragRef.current || !naturalSize || !imgRef.current) return;
      const { mode, startX, startY, startCrop } = dragRef.current;

      const rect = imgRef.current.getBoundingClientRect();
      const scaleX = naturalSize.width / rect.width;
      const scaleY = naturalSize.height / rect.height;

      const dx = Math.round((e.clientX - startX) * scaleX);
      const dy = Math.round((e.clientY - startY) * scaleY);

      setCrop(() => {
        let x = startCrop.x;
        let y = startCrop.y;
        let w = startCrop.width;
        let h = startCrop.height;

        if (mode === "move") {
          x = Math.max(0, Math.min(naturalSize.width - w, startCrop.x + dx));
          y = Math.max(0, Math.min(naturalSize.height - h, startCrop.y + dy));
          return { x, y, width: w, height: h };
        }

        // Resize handles
        if (mode.includes("e")) w = Math.max(20, startCrop.width + dx);
        if (mode.includes("s")) h = Math.max(20, startCrop.height + dy);
        if (mode.includes("w")) {
          const maxDelta = startCrop.width - 20;
          const actualDx = Math.min(maxDelta, dx);
          x = Math.max(0, startCrop.x + actualDx);
          w = startCrop.width - (x - startCrop.x);
        }
        if (mode.includes("n")) {
          const maxDelta = startCrop.height - 20;
          const actualDy = Math.min(maxDelta, dy);
          y = Math.max(0, startCrop.y + actualDy);
          h = startCrop.height - (y - startCrop.y);
        }

        // Keep within bounds
        w = Math.min(w, naturalSize.width - x);
        h = Math.min(h, naturalSize.height - y);

        return { x, y, width: w, height: h };
      });
    };

    const handleMouseUp = () => {
      dragRef.current = null;
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [naturalSize]);

  const handleReset = () => {
    if (!naturalSize) return;
    setAspectRatio("free");
    setCrop({
      x: 0,
      y: 0,
      width: naturalSize.width,
      height: naturalSize.height,
    });
    setSuccessPath(null);
  };

  const handleApply = async () => {
    if (isProcessing) return;
    setIsProcessing(true);
    try {
      const res = await invoke<string>("crop_image_file", {
        inputPath: filePath,
        x: Math.round(crop.x),
        y: Math.round(crop.y),
        width: Math.round(crop.width),
        height: Math.round(crop.height),
        outputPath: null,
      });
      setSuccessPath(res);
    } catch (e) {
      console.error("Crop failed:", e);
      alert(`Crop failed: ${e}`);
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <ToolWindowLayout
      title="Crop Image"
      subtitle={fileName}
      icon={<Crop size={16} />}
      primaryActionLabel="Apply Crop"
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleApply}
    >
      <div className="flex flex-col h-full gap-3 p-3">
        {/* Controls bar: Aspect Ratio presets & Pixel Dimensions */}
        <div className="flex flex-wrap items-center justify-between gap-3 px-3 py-2 rounded-xl bg-[#242424] border border-white/[0.06] text-xs shrink-0">
          {/* Ratio Segmented Control */}
          <RaycastSegmented
            value={aspectRatio}
            options={[
              { label: "Free", value: "free" },
              { label: "1:1", value: "1:1" },
              { label: "16:9", value: "16:9" },
              { label: "9:16", value: "9:16" },
              { label: "4:3", value: "4:3" },
              { label: "3:4", value: "3:4" },
            ]}
            onChange={(val) => applyAspectRatio(val as AspectRatio)}
          />

          {/* Pixel inputs: W x H and lock indicator */}
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5 text-neutral-400">
              <span className="text-[11px] font-mono text-neutral-500">W</span>
              <input
                type="number"
                value={Math.round(crop.width)}
                onChange={(e) => {
                  const val = Math.max(10, parseInt(e.target.value) || 10);
                  if (naturalSize) {
                    setCrop((c) => ({
                      ...c,
                      width: Math.min(val, naturalSize.width - c.x),
                    }));
                  }
                }}
                className="w-16 px-2 py-0.5 rounded-md bg-[#1c1c1c] border border-white/[0.08] text-right font-mono text-neutral-200 text-xs focus:outline-none focus:border-white/30"
              />
              <span className="text-[11px] text-neutral-600">×</span>
              <span className="text-[11px] font-mono text-neutral-500">H</span>
              <input
                type="number"
                value={Math.round(crop.height)}
                onChange={(e) => {
                  const val = Math.max(10, parseInt(e.target.value) || 10);
                  if (naturalSize) {
                    setCrop((c) => ({
                      ...c,
                      height: Math.min(val, naturalSize.height - c.y),
                    }));
                  }
                }}
                className="w-16 px-2 py-0.5 rounded-md bg-[#1c1c1c] border border-white/[0.08] text-right font-mono text-neutral-200 text-xs focus:outline-none focus:border-white/30"
              />
              <span className="text-[10px] text-neutral-500">px</span>
            </div>

            <div className="text-neutral-500 pl-2 border-l border-white/[0.08]">
              {aspectRatio !== "free" ? (
                <Lock size={12} className="text-[#ff6339]" />
              ) : (
                <Unlock size={12} className="text-neutral-600" />
              )}
            </div>
          </div>
        </div>

        {/* Live crop workspace container */}
        <div
          ref={containerRef}
          className="relative flex-1 flex items-center justify-center min-h-[360px] rounded-xl overflow-hidden bg-zinc-950/80 border border-white/5 p-4"
        >
          <div className="relative max-h-full max-w-full select-none inline-block">
            {/* Base Image */}
            <img
              ref={imgRef}
              src={assetUrl}
              alt="Crop preview"
              onLoad={handleImageLoad}
              className="max-h-[500px] max-w-full object-contain pointer-events-none rounded shadow-md"
            />

            {/* Interactive Crop Overlay */}
            {naturalSize && (
              <div className="absolute inset-0 pointer-events-auto">
                {/* Darkened backdrop surrounding crop box */}
                <div
                  className="absolute inset-0 bg-black/60 pointer-events-none"
                  style={{
                    clipPath: `polygon(
                      0% 0%, 100% 0%, 100% 100%, 0% 100%,
                      0% 0%,
                      ${toPercent(crop.x, naturalSize.width)}% ${toPercent(crop.y, naturalSize.height)}%,
                      ${toPercent(crop.x, naturalSize.width)}% ${toPercent(crop.y + crop.height, naturalSize.height)}%,
                      ${toPercent(crop.x + crop.width, naturalSize.width)}% ${toPercent(crop.y + crop.height, naturalSize.height)}%,
                      ${toPercent(crop.x + crop.width, naturalSize.width)}% ${toPercent(crop.y, naturalSize.height)}%,
                      ${toPercent(crop.x, naturalSize.width)}% ${toPercent(crop.y, naturalSize.height)}%
                    )`,
                  }}
                />

                {/* Crop Box Window */}
                <div
                  onMouseDown={(e) => handleMouseDown("move", e)}
                  style={{
                    left: `${toPercent(crop.x, naturalSize.width)}%`,
                    top: `${toPercent(crop.y, naturalSize.height)}%`,
                    width: `${toPercent(crop.width, naturalSize.width)}%`,
                    height: `${toPercent(crop.height, naturalSize.height)}%`,
                  }}
                  className="absolute border-2 border-orange-500 shadow-2xl cursor-move select-none"
                >
                  {/* Rule-of-thirds grid lines */}
                  <div className="absolute inset-0 grid grid-cols-3 grid-rows-3 pointer-events-none opacity-40">
                    <div className="border-r border-b border-white/40" />
                    <div className="border-r border-b border-white/40" />
                    <div className="border-b border-white/40" />
                    <div className="border-r border-b border-white/40" />
                    <div className="border-r border-b border-white/40" />
                    <div className="border-b border-white/40" />
                    <div className="border-r border-white/40" />
                    <div className="border-r border-white/40" />
                    <div />
                  </div>

                  {/* Corner handles */}
                  <div
                    onMouseDown={(e) => handleMouseDown("nw", e)}
                    className="absolute -top-1.5 -left-1.5 w-3.5 h-3.5 bg-orange-400 border border-white rounded-xs cursor-nwse-resize shadow-md"
                  />
                  <div
                    onMouseDown={(e) => handleMouseDown("ne", e)}
                    className="absolute -top-1.5 -right-1.5 w-3.5 h-3.5 bg-orange-400 border border-white rounded-xs cursor-nesw-resize shadow-md"
                  />
                  <div
                    onMouseDown={(e) => handleMouseDown("sw", e)}
                    className="absolute -bottom-1.5 -left-1.5 w-3.5 h-3.5 bg-orange-400 border border-white rounded-xs cursor-nesw-resize shadow-md"
                  />
                  <div
                    onMouseDown={(e) => handleMouseDown("se", e)}
                    className="absolute -bottom-1.5 -right-1.5 w-3.5 h-3.5 bg-orange-400 border border-white rounded-xs cursor-nwse-resize shadow-md"
                  />

                  {/* Edge handles */}
                  <div
                    onMouseDown={(e) => handleMouseDown("n", e)}
                    className="absolute -top-1 left-1/2 -translate-x-1/2 w-6 h-2 bg-orange-400 border border-white rounded-xs cursor-ns-resize shadow-md"
                  />
                  <div
                    onMouseDown={(e) => handleMouseDown("s", e)}
                    className="absolute -bottom-1 left-1/2 -translate-x-1/2 w-6 h-2 bg-orange-400 border border-white rounded-xs cursor-ns-resize shadow-md"
                  />
                  <div
                    onMouseDown={(e) => handleMouseDown("w", e)}
                    className="absolute -left-1 top-1/2 -translate-y-1/2 w-2 h-6 bg-orange-400 border border-white rounded-xs cursor-ew-resize shadow-md"
                  />
                  <div
                    onMouseDown={(e) => handleMouseDown("e", e)}
                    className="absolute -right-1 top-1/2 -translate-y-1/2 w-2 h-6 bg-orange-400 border border-white rounded-xs cursor-ew-resize shadow-md"
                  />
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </ToolWindowLayout>
  );
}
