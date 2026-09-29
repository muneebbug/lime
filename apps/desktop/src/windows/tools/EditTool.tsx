import { useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import {
  Sliders,
  RotateCw,
  RotateCcw,
  FlipHorizontal,
  FlipVertical,
  Lock,
  Unlock,
  Sun,
  Contrast,
  Thermometer,
} from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";

export interface EditToolProps {
  filePath: string;
}

export function EditTool({ filePath }: EditToolProps) {
  const [brightness, setBrightness] = useState<number>(0);
  const [contrast, setContrast] = useState<number>(0);
  const [saturation, setSaturation] = useState<number>(0);
  const [temperature, setTemperature] = useState<number>(0);
  const [rotation, setRotation] = useState<number>(0);
  const [flipH, setFlipH] = useState<boolean>(false);
  const [flipV, setFlipV] = useState<boolean>(false);
  const [naturalSize, setNaturalSize] = useState<{ width: number; height: number } | null>(null);
  const [resizeW, setResizeW] = useState<number>(0);
  const [resizeH, setResizeH] = useState<number>(0);
  const [lockAspect, setLockAspect] = useState<boolean>(true);

  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);

  const assetUrl = convertFileSrc(filePath);
  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  const handleImageLoad = (e: React.SyntheticEvent<HTMLImageElement>) => {
    const nw = e.currentTarget.naturalWidth;
    const nh = e.currentTarget.naturalHeight;
    setNaturalSize({ width: nw, height: nh });
    setResizeW(nw);
    setResizeH(nh);
  };

  const handleRotate = (delta: number) => {
    setRotation((r) => (r + delta + 360) % 360);
  };

  const handleWidthChange = (val: number) => {
    setResizeW(val);
    if (lockAspect && naturalSize && naturalSize.width > 0) {
      const ratio = naturalSize.height / naturalSize.width;
      setResizeH(Math.round(val * ratio));
    }
  };

  const handleHeightChange = (val: number) => {
    setResizeH(val);
    if (lockAspect && naturalSize && naturalSize.height > 0) {
      const ratio = naturalSize.width / naturalSize.height;
      setResizeW(Math.round(val * ratio));
    }
  };

  const handleReset = () => {
    setBrightness(0);
    setContrast(0);
    setSaturation(0);
    setTemperature(0);
    setRotation(0);
    setFlipH(false);
    setFlipV(false);
    if (naturalSize) {
      setResizeW(naturalSize.width);
      setResizeH(naturalSize.height);
    }
    setSuccessPath(null);
  };

  const handleApply = async () => {
    if (isProcessing) return;
    setIsProcessing(true);
    try {
      const res = await invoke<string>("edit_image_file", {
        inputPath: filePath,
        brightness,
        contrast: contrast as number,
        rotation,
        flipH,
        flipV,
        resizeW: naturalSize && resizeW !== naturalSize.width ? resizeW : null,
        resizeH: naturalSize && resizeH !== naturalSize.height ? resizeH : null,
        outputPath: null,
      });
      setSuccessPath(res);
    } catch (e) {
      console.error("Edit failed:", e);
      alert(`Edit failed: ${e}`);
    } finally {
      setIsProcessing(false);
    }
  };

  // Compute CSS filter for real-time live preview
  const previewFilter = `
    brightness(${1 + brightness / 100})
    contrast(${1 + contrast / 100})
    saturate(${1 + saturation / 100})
    sepia(${temperature > 0 ? temperature / 200 : 0})
    hue-rotate(${temperature < 0 ? (temperature / 100) * 20 : 0}deg)
  `.trim();

  const previewTransform = `
    rotate(${rotation}deg)
    scaleX(${flipH ? -1 : 1})
    scaleY(${flipV ? -1 : 1})
  `.trim();

  return (
    <ToolWindowLayout
      title="Edit Image"
      subtitle={fileName}
      icon={<Sliders size={16} className="text-[#ff6339]" />}
      primaryActionLabel="Apply Adjustments"
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleApply}
    >
      <div className="flex flex-col h-full gap-4">
        {/* Controls Toolbar: Transform & Adjustments */}
        <div className="flex flex-col gap-3.5 p-4 rounded-xl bg-[#242424] border border-white/[0.06] text-xs shadow-sm">
          {/* Top row: Rotation & Flips + Resize inputs */}
          <div className="flex items-center justify-between gap-3 pb-3 border-b border-white/[0.06]">
            {/* Rotate & Flip buttons */}
            <div className="flex items-center gap-1.5">
              <span className="text-[11px] text-neutral-400 font-medium uppercase tracking-wider mr-1">
                Transform:
              </span>
              <button
                onClick={() => handleRotate(-90)}
                className="p-1.5 rounded-lg bg-[#1c1c1c] border border-white/[0.08] hover:bg-white/[0.08] text-neutral-300 transition-colors cursor-pointer"
                title="Rotate 90° CCW"
              >
                <RotateCcw size={14} />
              </button>
              <button
                onClick={() => handleRotate(90)}
                className="p-1.5 rounded-lg bg-[#1c1c1c] border border-white/[0.08] hover:bg-white/[0.08] text-neutral-300 transition-colors cursor-pointer"
                title="Rotate 90° CW"
              >
                <RotateCw size={14} />
              </button>
              <div className="w-px h-5 bg-white/[0.08] mx-1" />
              <button
                onClick={() => setFlipH(!flipH)}
                className={`p-1.5 rounded-lg border transition-colors cursor-pointer ${
                  flipH
                    ? "bg-white/[0.14] border-white/30 text-white font-medium shadow-sm"
                    : "bg-[#1c1c1c] border-white/[0.08] hover:bg-white/[0.08] text-neutral-300"
                }`}
                title="Flip Horizontal"
              >
                <FlipHorizontal size={14} />
              </button>
              <button
                onClick={() => setFlipV(!flipV)}
                className={`p-1.5 rounded-lg border transition-colors cursor-pointer ${
                  flipV
                    ? "bg-white/[0.14] border-white/30 text-white font-medium shadow-sm"
                    : "bg-[#1c1c1c] border-white/[0.08] hover:bg-white/[0.08] text-neutral-300"
                }`}
                title="Flip Vertical"
              >
                <FlipVertical size={14} />
              </button>
            </div>

            {/* Resize inputs */}
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-neutral-400 font-medium uppercase tracking-wider">
                Resize:
              </span>
              <div className="flex items-center gap-1.5">
                <input
                  type="number"
                  value={resizeW}
                  onChange={(e) => handleWidthChange(parseInt(e.target.value) || 1)}
                  className="w-18 px-2 py-1 rounded-lg bg-[#1c1c1c] border border-white/[0.08] text-right font-mono text-xs text-neutral-200 focus:outline-none focus:border-white/30"
                />
                <span className="text-neutral-500 font-mono">×</span>
                <input
                  type="number"
                  value={resizeH}
                  onChange={(e) => handleHeightChange(parseInt(e.target.value) || 1)}
                  className="w-18 px-2 py-1 rounded-lg bg-[#1c1c1c] border border-white/[0.08] text-right font-mono text-xs text-neutral-200 focus:outline-none focus:border-white/30"
                />
                <button
                  onClick={() => setLockAspect(!lockAspect)}
                  className="p-1 rounded text-neutral-400 hover:text-neutral-200 cursor-pointer ml-0.5"
                  title={lockAspect ? "Aspect ratio locked" : "Aspect ratio unlocked"}
                >
                  {lockAspect ? (
                    <Lock size={13} className="text-[#ff6339]" />
                  ) : (
                    <Unlock size={13} className="text-neutral-500" />
                  )}
                </button>
              </div>
            </div>
          </div>

          {/* Adjustment Sliders Grid */}
          <div className="grid grid-cols-4 gap-4 pt-1">
            {/* Brightness */}
            <div className="flex flex-col gap-1.5">
              <div className="flex justify-between text-[11px] text-neutral-400 font-medium">
                <span className="flex items-center gap-1">
                  <Sun size={12} className="text-neutral-400" /> Brightness
                </span>
                <span className="font-mono text-neutral-200">{brightness > 0 ? `+${brightness}` : brightness}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={brightness}
                onChange={(e) => setBrightness(parseInt(e.target.value) || 0)}
                className="accent-[#ff6339] cursor-pointer h-1.5 bg-[#181818] rounded"
              />
            </div>

            {/* Contrast */}
            <div className="flex flex-col gap-1.5">
              <div className="flex justify-between text-[11px] text-neutral-400 font-medium">
                <span className="flex items-center gap-1">
                  <Contrast size={12} className="text-neutral-400" /> Contrast
                </span>
                <span className="font-mono text-neutral-200">{contrast > 0 ? `+${contrast}` : contrast}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={contrast}
                onChange={(e) => setContrast(parseInt(e.target.value) || 0)}
                className="accent-[#ff6339] cursor-pointer h-1.5 bg-[#181818] rounded"
              />
            </div>

            {/* Saturation */}
            <div className="flex flex-col gap-1.5">
              <div className="flex justify-between text-[11px] text-neutral-400 font-medium">
                <span>Saturation</span>
                <span className="font-mono text-neutral-200">{saturation > 0 ? `+${saturation}` : saturation}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={saturation}
                onChange={(e) => setSaturation(parseInt(e.target.value) || 0)}
                className="accent-[#ff6339] cursor-pointer h-1.5 bg-[#181818] rounded"
              />
            </div>

            {/* Temperature */}
            <div className="flex flex-col gap-1.5">
              <div className="flex justify-between text-[11px] text-neutral-400 font-medium">
                <span className="flex items-center gap-1">
                  <Thermometer size={12} className="text-neutral-400" /> Warmth
                </span>
                <span className="font-mono text-neutral-200">{temperature > 0 ? `+${temperature}` : temperature}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={temperature}
                onChange={(e) => setTemperature(parseInt(e.target.value) || 0)}
                className="accent-[#ff6339] cursor-pointer h-1.5 bg-[#181818] rounded"
              />
            </div>
          </div>
        </div>

        {/* Live Preview Canvas */}
        <div className="flex-1 min-h-[300px] rounded-xl overflow-hidden bg-[#181818] border border-white/[0.06] flex items-center justify-center p-6 relative shadow-inner">
          <img
            src={assetUrl}
            alt="Edit preview"
            onLoad={handleImageLoad}
            style={{
              filter: previewFilter,
              transform: previewTransform,
              transition: "filter 0.05s ease-out, transform 0.15s ease-out",
            }}
            className="max-h-[360px] max-w-full object-contain rounded shadow-lg select-none"
          />
        </div>
      </div>
    </ToolWindowLayout>
  );
}
