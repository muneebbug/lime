import { useState, useEffect } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Minimize2, Zap, ShieldCheck, Target, ArrowDownRight } from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";
import { ToggleSwitch } from "../../ui/WheelUI";

export interface CompressToolProps {
  filePath: string;
}

export function CompressTool({ filePath }: CompressToolProps) {
  const [fileSize, setFileSize] = useState<number | null>(null);
  const [preset, setPreset] = useState<"balanced" | "strong">("balanced");
  const [useTargetSize, setUseTargetSize] = useState(false);
  const [targetSizeKb, setTargetSizeKb] = useState<number>(500);
  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);
  const [compressedSize, setCompressedSize] = useState<number | null>(null);

  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;
  const ext = fileName.split(".").pop()?.toUpperCase() ?? "FILE";
  const assetUrl = convertFileSrc(filePath);

  useEffect(() => {
    // Fetch original file metadata
    invoke<{ file_size: number }>("get_image_metadata", { inputPath: filePath })
      .then((meta) => {
        setFileSize(meta.file_size);
        setTargetSizeKb(Math.max(50, Math.round((meta.file_size / 1024) * 0.4)));
      })
      .catch((e) => console.error("Failed to get file metadata", e));
  }, [filePath]);

  const formatSize = (bytes: number | null) => {
    if (bytes === null) return "...";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  };

  // Estimate compressed size
  const estimatedBytes = fileSize
    ? useTargetSize
      ? targetSizeKb * 1024
      : preset === "strong"
      ? Math.round(fileSize * 0.35)
      : Math.round(fileSize * 0.65)
    : null;

  const estimatedSavedPercent = fileSize && estimatedBytes
    ? Math.max(0, Math.round(((fileSize - estimatedBytes) / fileSize) * 100))
    : 0;

  const handleReset = () => {
    setPreset("balanced");
    setUseTargetSize(false);
    setSuccessPath(null);
    setCompressedSize(null);
    if (fileSize) {
      setTargetSizeKb(Math.max(50, Math.round((fileSize / 1024) * 0.4)));
    }
  };

  const handleCompress = async () => {
    if (isProcessing) return;
    setIsProcessing(true);
    try {
      const res = await invoke<string>("compress_image_file", {
        inputPath: filePath,
        preset,
        targetSizeKb: useTargetSize ? targetSizeKb : null,
        outputPath: null,
      });

      // Get resulting file size
      try {
        const outMeta = await invoke<{ file_size: number }>("get_image_metadata", {
          inputPath: res,
        });
        setCompressedSize(outMeta.file_size);
      } catch {
        // Ignored
      }

      setSuccessPath(res);
    } catch (e) {
      console.error("Compression failed:", e);
      alert(`Compression failed: ${e}`);
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <ToolWindowLayout
      title="Compress"
      subtitle={fileName}
      icon={<Minimize2 size={14} />}
      primaryActionLabel="Compress File"
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleCompress}
    >
      <div className="flex flex-col h-full gap-4 p-5 max-w-xl mx-auto w-full">
        {/* File Summary Card */}
        <div className="grid grid-cols-3 gap-3 p-3 rounded-xl bg-[#242424] border border-white/[0.06] text-xs">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-[#2e2e2e] text-[#ff6339] flex items-center justify-center font-bold text-[10px] tracking-wider border border-white/[0.08]">
              {ext}
            </div>
            <div>
              <div className="text-[10px] text-neutral-500 uppercase tracking-wider font-semibold">
                Original
              </div>
              <div className="text-neutral-200 font-mono font-medium">
                {formatSize(fileSize)}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2.5 border-l border-white/[0.06] pl-3">
            <div className="w-8 h-8 rounded-lg bg-emerald-500/10 text-emerald-400 flex items-center justify-center border border-emerald-500/20">
              <ArrowDownRight size={15} />
            </div>
            <div>
              <div className="text-[10px] text-neutral-500 uppercase tracking-wider font-semibold">
                {compressedSize !== null ? "Final" : "Estimated"}
              </div>
              <div className="text-emerald-400 font-mono font-medium">
                {formatSize(compressedSize ?? estimatedBytes)}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2.5 border-l border-white/[0.06] pl-3">
            <div className="w-8 h-8 rounded-lg bg-[#ff6339]/10 text-[#ff6339] flex items-center justify-center border border-[#ff6339]/20 font-bold text-xs">
              %
            </div>
            <div>
              <div className="text-[10px] text-neutral-500 uppercase tracking-wider font-semibold">
                Reduction
              </div>
              <div className="text-[#ff6339] font-semibold font-mono">
                {compressedSize !== null && fileSize
                  ? `-${Math.max(0, Math.round(((fileSize - compressedSize) / fileSize) * 100))}%`
                  : `~${estimatedSavedPercent}%`}
              </div>
            </div>
          </div>
        </div>

        {/* Compression Preset Selector */}
        <div className="flex flex-col gap-2">
          <label className="text-[11px] font-semibold uppercase tracking-wider text-neutral-400">
            Compression Preset
          </label>
          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={() => {
                setPreset("balanced");
                setUseTargetSize(false);
              }}
              className={`flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-default ${
                preset === "balanced" && !useTargetSize
                  ? "bg-white/[0.08] border-white/20 text-white shadow-sm"
                  : "bg-[#242424] border-white/[0.06] text-neutral-400 hover:text-neutral-200 hover:bg-[#2a2a2a]"
              }`}
            >
              <ShieldCheck
                size={18}
                className={
                  preset === "balanced" && !useTargetSize
                    ? "text-[#ff6339] shrink-0 mt-0.5"
                    : "text-neutral-500 shrink-0 mt-0.5"
                }
              />
              <div>
                <div className="text-xs font-semibold text-neutral-200">Balanced</div>
                <div className="text-[11px] text-neutral-400 mt-0.5 leading-normal">
                  Quality preservation with ~35% smaller file size.
                </div>
              </div>
            </button>

            <button
              onClick={() => {
                setPreset("strong");
                setUseTargetSize(false);
              }}
              className={`flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-default ${
                preset === "strong" && !useTargetSize
                  ? "bg-white/[0.08] border-white/20 text-white shadow-sm"
                  : "bg-[#242424] border-white/[0.06] text-neutral-400 hover:text-neutral-200 hover:bg-[#2a2a2a]"
              }`}
            >
              <Zap
                size={18}
                className={
                  preset === "strong" && !useTargetSize
                    ? "text-[#ff6339] shrink-0 mt-0.5"
                    : "text-neutral-500 shrink-0 mt-0.5"
                }
              />
              <div>
                <div className="text-xs font-semibold text-neutral-200">Strong</div>
                <div className="text-[11px] text-neutral-400 mt-0.5 leading-normal">
                  Smallest possible size (~65% reduction) with resolution cap.
                </div>
              </div>
            </button>
          </div>
        </div>

        {/* Target File Size Option */}
        <div className="flex flex-col gap-2.5 p-3.5 rounded-xl bg-[#242424] border border-white/[0.06]">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-2 text-xs font-medium text-neutral-200">
              <Target size={14} className="text-[#ff6339]" />
              Compress to target file size
            </span>
            <div className="flex items-center gap-3">
              {useTargetSize && (
                <span className="text-[11px] font-mono text-[#ff6339] font-semibold">
                  {targetSizeKb >= 1024
                    ? `${(targetSizeKb / 1024).toFixed(2)} MB`
                    : `${targetSizeKb} KB`}
                </span>
              )}
              <ToggleSwitch
                checked={useTargetSize}
                onChange={setUseTargetSize}
              />
            </div>
          </div>

          {useTargetSize && (
            <div className="flex items-center gap-4 mt-2 pt-2 border-t border-white/[0.04]">
              <input
                type="range"
                min="20"
                max={fileSize ? Math.round(fileSize / 1024) : 2048}
                value={targetSizeKb}
                onChange={(e) => setTargetSizeKb(parseInt(e.target.value))}
                className="flex-1 accent-[#ff6339] cursor-default"
              />
              <div className="flex items-center gap-1">
                <input
                  type="number"
                  value={targetSizeKb}
                  onChange={(e) => setTargetSizeKb(Math.max(10, parseInt(e.target.value) || 10))}
                  className="w-20 px-2 py-1 rounded-md bg-[#1c1c1c] border border-white/[0.08] text-right font-mono text-xs text-neutral-200 focus:outline-none focus:border-white/30"
                />
                <span className="text-xs text-neutral-500 font-mono">KB</span>
              </div>
            </div>
          )}
        </div>

        {/* Live Preview Display */}
        <div className="flex-1 min-h-[160px] rounded-xl overflow-hidden bg-[#181818] border border-white/[0.06] flex items-center justify-center p-3 relative">
          {assetUrl ? (
            <img
              src={assetUrl}
              alt="Preview"
              className="max-h-[220px] max-w-full object-contain rounded shadow"
            />
          ) : (
            <div className="flex flex-col items-center gap-2 text-neutral-600">
              <span className="text-xs">No image preview available</span>
            </div>
          )}
        </div>
      </div>
    </ToolWindowLayout>
  );
}
