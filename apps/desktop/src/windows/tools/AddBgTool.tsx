import { useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Image as ImageIcon, Palette } from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";
import { SegmentedControl } from "../../ui/WheelUI";

export interface AddBgToolProps {
  filePath: string;
}

interface GradientPreset {
  name: string;
  start: [number, number, number, number];
  end: [number, number, number, number];
  css: string;
}

const GRADIENT_PRESETS: GradientPreset[] = [
  {
    name: "Warm Sunset",
    start: [249, 115, 22, 255],
    end: [236, 72, 153, 255],
    css: "linear-gradient(135deg, #f97316 0%, #ec4899 100%)",
  },
  {
    name: "Neon Violet",
    start: [168, 85, 247, 255],
    end: [59, 130, 246, 255],
    css: "linear-gradient(135deg, #a855f7 0%, #3b82f6 100%)",
  },
  {
    name: "Cyber Ocean",
    start: [6, 182, 212, 255],
    end: [59, 130, 246, 255],
    css: "linear-gradient(135deg, #06b6d4 0%, #3b82f6 100%)",
  },
  {
    name: "Aurora Green",
    start: [16, 185, 129, 255],
    end: [6, 182, 212, 255],
    css: "linear-gradient(135deg, #10b981 0%, #06b6d4 100%)",
  },
  {
    name: "Midnight Dark",
    start: [24, 24, 27, 255],
    end: [9, 9, 11, 255],
    css: "linear-gradient(135deg, #27272a 0%, #09090b 100%)",
  },
  {
    name: "Rose Peach",
    start: [251, 113, 133, 255],
    end: [253, 186, 116, 255],
    css: "linear-gradient(135deg, #fb7185 0%, #fdba74 100%)",
  },
];

const SOLID_PRESETS = [
  { name: "White", rgba: [255, 255, 255, 255], hex: "#ffffff" },
  { name: "Light Gray", rgba: [244, 244, 245, 255], hex: "#f4f4f5" },
  { name: "Zinc", rgba: [63, 63, 70, 255], hex: "#3f3f46" },
  { name: "Dark", rgba: [24, 24, 27, 255], hex: "#18181b" },
  { name: "Black", rgba: [0, 0, 0, 255], hex: "#000000" },
  { name: "Orange", rgba: [249, 115, 22, 255], hex: "#f97316" },
  { name: "Blue", rgba: [59, 130, 246, 255], hex: "#3b82f6" },
];

export function AddBgTool({ filePath }: AddBgToolProps) {
  const [selectedGradient, setSelectedGradient] = useState<GradientPreset>(GRADIENT_PRESETS[0]);
  const [padding, setPadding] = useState<number>(48);
  const [cornerRadius, setCornerRadius] = useState<number>(16);
  const [shadowBlur, setShadowBlur] = useState<number>(24);
  const [aspectRatio, setAspectRatio] = useState<"auto" | "1:1" | "16:9" | "4:5">("auto");
  const [exportFormat, setExportFormat] = useState<"png" | "jpg" | "webp">("png");
  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);

  const assetUrl = convertFileSrc(filePath);
  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  const handleReset = () => {
    setSelectedGradient(GRADIENT_PRESETS[0]);
    setPadding(48);
    setCornerRadius(16);
    setShadowBlur(24);
    setAspectRatio("auto");
    setExportFormat("png");
    setSuccessPath(null);
  };

  const handleApply = async () => {
    if (isProcessing) return;
    setIsProcessing(true);
    try {
      const res = await invoke<string>("add_background_file", {
        inputPath: filePath,
        padding,
        cornerRadius,
        shadowBlur,
        aspectRatio,
        colorStart: selectedGradient.start,
        colorEnd: selectedGradient.end,
        format: exportFormat,
        outputPath: null,
      });
      setSuccessPath(res);
    } catch (e) {
      console.error("Add BG failed:", e);
      alert(`Add background failed: ${e}`);
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <ToolWindowLayout
      title="Add Background"
      subtitle={fileName}
      icon={<ImageIcon size={16} />}
      primaryActionLabel="Save with Background"
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleApply}
    >
      <div className="flex flex-col h-full gap-3 p-3">
        {/* Controls Grid */}
        <div className="flex flex-col gap-3 p-3 rounded-xl bg-[#242424] border border-white/[0.06] text-xs shrink-0">
          {/* Swatches selector: Gradients and Solids */}
          <div className="flex flex-col gap-1.5">
            <div className="flex items-center justify-between text-[11px] text-neutral-400 font-semibold uppercase tracking-wider">
              <span className="flex items-center gap-1.5">
                <Palette size={13} />
                Background Swatches
              </span>
              <span className="text-neutral-500 font-normal lowercase">{selectedGradient.name}</span>
            </div>

            {/* Gradient row */}
            <div className="flex items-center gap-2 overflow-x-auto pb-1">
              {GRADIENT_PRESETS.map((g) => (
                <button
                  key={g.name}
                  onClick={() => setSelectedGradient(g)}
                  style={{ background: g.css }}
                  className={`w-6 h-6 rounded-full shrink-0 transition-transform cursor-default shadow-sm ${
                    selectedGradient.name === g.name
                      ? "ring-2 ring-[#ff6339] scale-110 ring-offset-2 ring-offset-[#181818]"
                      : "opacity-80 hover:opacity-100 hover:scale-105"
                  }`}
                  title={g.name}
                />
              ))}
              <div className="w-px h-5 bg-white/[0.08] mx-1 shrink-0" />
              {/* Solid row */}
              {SOLID_PRESETS.map((s) => (
                <button
                  key={s.name}
                  onClick={() =>
                    setSelectedGradient({
                      name: s.name,
                      start: s.rgba as [number, number, number, number],
                      end: s.rgba as [number, number, number, number],
                      css: s.hex,
                    })
                  }
                  style={{ backgroundColor: s.hex }}
                  className={`w-6 h-6 rounded-full shrink-0 border border-white/15 transition-transform cursor-default shadow-sm ${
                    selectedGradient.css === s.hex
                      ? "ring-2 ring-[#ff6339] scale-110 ring-offset-2 ring-offset-[#181818]"
                      : "opacity-80 hover:opacity-100 hover:scale-105"
                  }`}
                  title={s.name}
                />
              ))}
            </div>
          </div>

          {/* Sliders: Padding, Corner Radius, Shadow */}
          <div className="grid grid-cols-3 gap-4 pt-2 border-t border-white/[0.04]">
            {/* Padding slider */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between text-[11px] text-neutral-400 font-medium">
                <span>Padding</span>
                <span className="font-mono text-neutral-200">{padding}px</span>
              </div>
              <input
                type="range"
                min={0}
                max={160}
                value={padding}
                onChange={(e) => setPadding(parseInt(e.target.value) || 0)}
                className="accent-[#ff6339] cursor-default h-1.5 bg-[#1c1c1c] rounded"
              />
            </div>

            {/* Corner Radius slider */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between text-[11px] text-neutral-400 font-medium">
                <span>Corners</span>
                <span className="font-mono text-neutral-200">{cornerRadius}px</span>
              </div>
              <input
                type="range"
                min={0}
                max={48}
                value={cornerRadius}
                onChange={(e) => setCornerRadius(parseInt(e.target.value) || 0)}
                className="accent-[#ff6339] cursor-default h-1.5 bg-[#1c1c1c] rounded"
              />
            </div>

            {/* Shadow slider */}
            <div className="flex flex-col gap-1">
              <div className="flex justify-between text-[11px] text-neutral-400 font-medium">
                <span>Shadow</span>
                <span className="font-mono text-neutral-200">{shadowBlur}px</span>
              </div>
              <input
                type="range"
                min={0}
                max={60}
                value={shadowBlur}
                onChange={(e) => setShadowBlur(parseInt(e.target.value) || 0)}
                className="accent-[#ff6339] cursor-default h-1.5 bg-[#1c1c1c] rounded"
              />
            </div>
          </div>

          {/* Ratio & Format Selector */}
          <div className="flex items-center justify-between pt-2 border-t border-white/[0.04]">
            {/* Aspect Ratio */}
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-neutral-400 font-medium">Ratio:</span>
              <SegmentedControl
                value={aspectRatio}
                options={[
                  { label: "Auto", value: "auto" },
                  { label: "1:1", value: "1:1" },
                  { label: "16:9", value: "16:9" },
                  { label: "4:5", value: "4:5" },
                ]}
                onChange={(val) => setAspectRatio(val as any)}
              />
            </div>

            {/* Export Format */}
            <div className="flex items-center gap-2">
              <span className="text-[11px] text-neutral-400 font-medium">Format:</span>
              <SegmentedControl
                value={exportFormat}
                options={[
                  { label: "PNG", value: "png" },
                  { label: "JPG", value: "jpg" },
                  { label: "WEBP", value: "webp" },
                ]}
                onChange={(val) => setExportFormat(val as any)}
              />
            </div>
          </div>
        </div>

        {/* Live Preview Display */}
        <div className="flex-1 min-h-[300px] rounded-xl overflow-hidden bg-zinc-950/80 border border-white/5 flex items-center justify-center p-6">
          <div
            style={{
              background: selectedGradient.css,
              padding: `${Math.round(padding * 0.7)}px`,
              aspectRatio:
                aspectRatio === "1:1"
                  ? "1/1"
                  : aspectRatio === "16:9"
                  ? "16/9"
                  : aspectRatio === "4:5"
                  ? "4/5"
                  : undefined,
            }}
            className="flex items-center justify-center rounded-2xl max-h-[380px] max-w-full transition-all duration-150"
          >
            <img
              src={assetUrl}
              alt="Backdrop preview"
              style={{
                borderRadius: `${cornerRadius}px`,
                boxShadow:
                  shadowBlur > 0
                    ? `0 ${shadowBlur * 0.7}px ${shadowBlur * 1.5}px rgba(0, 0, 0, 0.5)`
                    : "none",
              }}
              className="max-h-[280px] max-w-full object-contain transition-all duration-150"
            />
          </div>
        </div>
      </div>
    </ToolWindowLayout>
  );
}
