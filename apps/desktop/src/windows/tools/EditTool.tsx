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
      icon={<Sliders size={16} />}
      primaryActionLabel="Apply Adjustments"
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleApply}
    >
      <div className="flex flex-col h-full gap-4">
        {/* Controls Toolbar: Transform & Adjustments */}
        <div className="flex flex-col gap-3 p-3.5 rounded-xl bg-zinc-900/60 border border-white/5 text-xs">
          {/* Top row: Rotation & Flips + Resize inputs */}
          <div className="flex items-center justify-between gap-3 pb-2 border-b border-white/5">
            {/* Rotate & Flip buttons */}
            <div className="flex items-center gap-1.5">
              <span className="text-[11px] text-zinc-500 font-semibold uppercase tracking-wider mr-1">
                Transform:
              </span>
              <button
                onClick={() => handleRotate(-90)}
                className="p-1.5 rounded-lg bg-zinc-950 border border-white/10 hover:bg-white/5 text-zinc-300 transition-colors cursor-pointer"
                title="Rotate 90° CCW"
              >
                <RotateCcw size={14} />
              </button>
              <button
                onClick={() => handleRotate(90)}
                className="p-1.5 rounded-lg bg-zinc-950 border border-white/10 hover:bg-white/5 text-zinc-300 transition-colors cursor-pointer"
                title="Rotate 90° CW"
              >
                <RotateCw size={14} />
              </button>
              <div className="w-px h-5 bg-white/10 mx-1" />
              <button
                onClick={() => setFlipH(!flipH)}
                className={`p-1.5 rounded-lg border transition-colors cursor-pointer ${
                  flipH
                    ? "bg-orange-500/20 border-orange-500 text-orange-300"
                    : "bg-zinc-950 border-white/10 hover:bg-white/5 text-zinc-300"
                }`}
                title="Flip Horizontal"
              >
                <FlipHorizontal size={14} />
              </button>
              <button
                onClick={() => setFlipV(!flipV)}
                className={`p-1.5 rounded-lg border transition-colors cursor-pointer ${
                  flipV
                    ? "bg-orange-500/20 border-orange-500 text-orange-300"
                    : "bg-zinc-950 border-white/10 hover:bg-white/5 text-zinc-300"
                }`}
                title="Flip Vertical"
              >
                <FlipVertical size={14} />
              </button>
            </div>

            {/* Resize inputs */}
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-zinc-500 font-semibold uppercase tracking-wider">
                Resize:
              </span>
              <div className="flex items-center gap-1">
                <input
                  type="number"
                  value={resizeW}
                  onChange={(e) => handleWidthChange(parseInt(e.target.value) || 1)}
                  className="w-16 px-1.5 py-0.5 rounded bg-zinc-950 border border-white/10 text-right font-mono text-[11px] text-zinc-200 focus:outline-none focus:border-orange-500"
                />
                <span className="text-zinc-600">×</span>
                <input
                  type="number"
                  value={resizeH}
                  onChange={(e) => handleHeightChange(parseInt(e.target.value) || 1)}
                  className="w-16 px-1.5 py-0.5 rounded bg-zinc-950 border border-white/10 text-right font-mono text-[11px] text-zinc-200 focus:outline-none focus:border-orange-500"
                />
                <button
                  onClick={() => setLockAspect(!lockAspect)}
                  className="p-1 rounded text-zinc-400 hover:text-zinc-200 cursor-pointer ml-0.5"
                  title={lockAspect ? "Aspect ratio locked" : "Aspect ratio unlocked"}
                >
                  {lockAspect ? (
                    <Lock size={12} className="text-orange-400" />
                  ) : (
                    <Unlock size={12} className="text-zinc-600" />
                  )}
                </button>
              </div>
            </div>
          </div>

          {/* Adjustment Sliders Grid */}
          <div className="grid grid-cols-4 gap-4">
            {/* Brightness */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between text-[11px] text-zinc-400 font-medium">
                <span className="flex items-center gap-1">
                  <Sun size={12} /> Brightness
                </span>
                <span className="font-mono text-zinc-200">{brightness > 0 ? `+${brightness}` : brightness}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={brightness}
                onChange={(e) => setBrightness(parseInt(e.target.value) || 0)}
                className="accent-orange-500 cursor-pointer h-1.5 bg-zinc-950 rounded"
              />
            </div>

            {/* Contrast */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between text-[11px] text-zinc-400 font-medium">
                <span className="flex items-center gap-1">
                  <Contrast size={12} /> Contrast
                </span>
                <span className="font-mono text-zinc-200">{contrast > 0 ? `+${contrast}` : contrast}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={contrast}
                onChange={(e) => setContrast(parseInt(e.target.value) || 0)}
                className="accent-orange-500 cursor-pointer h-1.5 bg-zinc-950 rounded"
              />
            </div>

            {/* Saturation */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between text-[11px] text-zinc-400 font-medium">
                <span>Saturation</span>
                <span className="font-mono text-zinc-200">{saturation > 0 ? `+${saturation}` : saturation}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={saturation}
                onChange={(e) => setSaturation(parseInt(e.target.value) || 0)}
                className="accent-orange-500 cursor-pointer h-1.5 bg-zinc-950 rounded"
              />
            </div>

            {/* Temperature */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between text-[11px] text-zinc-400 font-medium">
                <span className="flex items-center gap-1">
                  <Thermometer size={12} /> Warmth
                </span>
                <span className="font-mono text-zinc-200">{temperature > 0 ? `+${temperature}` : temperature}</span>
              </div>
              <input
                type="range"
                min={-100}
                max={100}
                value={temperature}
                onChange={(e) => setTemperature(parseInt(e.target.value) || 0)}
                className="accent-orange-500 cursor-pointer h-1.5 bg-zinc-950 rounded"
              />
            </div>
          </div>
        </div>

        {/* Live Preview Canvas */}
        <div className="flex-1 min-h-[300px] rounded-xl overflow-hidden bg-zinc-950/80 border border-white/5 flex items-center justify-center p-6 relative">
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
