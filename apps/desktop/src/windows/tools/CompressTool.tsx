import { useState, useEffect } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Minimize2, Zap, ShieldCheck, Target, ArrowDownRight, FileText } from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";

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

  const assetUrl = convertFileSrc(filePath);
  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;
  const ext = fileName.split(".").pop()?.toUpperCase() ?? "FILE";

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
      icon={<Minimize2 size={16} />}
      primaryActionLabel="Compress File"
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleCompress}
    >
      <div className="flex flex-col h-full gap-5">
        {/* File Summary Card */}
        <div className="grid grid-cols-3 gap-3 p-3.5 rounded-xl bg-zinc-900/60 border border-white/5 text-xs">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-orange-500/10 text-orange-400 flex items-center justify-center font-bold text-[10px] tracking-wider border border-orange-500/20">
              {ext}
            </div>
            <div>
              <div className="text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
                Original Size
              </div>
              <div className="text-zinc-200 font-mono font-medium">
                {formatSize(fileSize)}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2.5 border-l border-white/5 pl-3">
            <div className="w-8 h-8 rounded-lg bg-emerald-500/10 text-emerald-400 flex items-center justify-center border border-emerald-500/20">
              <ArrowDownRight size={16} />
            </div>
            <div>
              <div className="text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
                {compressedSize !== null ? "Final Size" : "Estimated Size"}
              </div>
              <div className="text-emerald-400 font-mono font-medium">
                {formatSize(compressedSize ?? estimatedBytes)}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2.5 border-l border-white/5 pl-3">
            <div className="w-8 h-8 rounded-lg bg-amber-500/10 text-amber-400 flex items-center justify-center border border-amber-500/20 font-bold text-xs">
              %
            </div>
            <div>
              <div className="text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
                Reduction
              </div>
              <div className="text-amber-400 font-semibold font-mono">
                {compressedSize !== null && fileSize
                  ? `-${Math.max(0, Math.round(((fileSize - compressedSize) / fileSize) * 100))}%`
                  : `~${estimatedSavedPercent}%`}
              </div>
            </div>
          </div>
        </div>

        {/* Compression Preset Selector */}
        <div className="flex flex-col gap-2">
          <label className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
            Compression Preset
          </label>
          <div className="grid grid-cols-2 gap-3">
            <button
              onClick={() => {
                setPreset("balanced");
                setUseTargetSize(false);
              }}
              className={`flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer ${
                preset === "balanced" && !useTargetSize
                  ? "bg-orange-500/10 border-orange-500/40 text-orange-200 shadow-md shadow-orange-950/20"
                  : "bg-zinc-900/40 border-white/5 text-zinc-400 hover:text-zinc-200 hover:bg-white/5"
              }`}
            >
              <ShieldCheck
                size={20}
                className={
                  preset === "balanced" && !useTargetSize
                    ? "text-orange-400 shrink-0 mt-0.5"
                    : "text-zinc-500 shrink-0 mt-0.5"
                }
              />
              <div>
                <div className="text-xs font-semibold text-zinc-200">Balanced</div>
                <div className="text-[11px] text-zinc-400 mt-0.5">
                  Great quality preservation with ~35% smaller file size.
                </div>
              </div>
            </button>

            <button
              onClick={() => {
                setPreset("strong");
                setUseTargetSize(false);
              }}
              className={`flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer ${
                preset === "strong" && !useTargetSize
                  ? "bg-orange-500/10 border-orange-500/40 text-orange-200 shadow-md shadow-orange-950/20"
                  : "bg-zinc-900/40 border-white/5 text-zinc-400 hover:text-zinc-200 hover:bg-white/5"
              }`}
            >
              <Zap
                size={20}
                className={
                  preset === "strong" && !useTargetSize
                    ? "text-orange-400 shrink-0 mt-0.5"
                    : "text-zinc-500 shrink-0 mt-0.5"
                }
              />
              <div>
                <div className="text-xs font-semibold text-zinc-200">Strong</div>
                <div className="text-[11px] text-zinc-400 mt-0.5">
                  Smallest possible size (~65% reduction) with resolution cap.
                </div>
              </div>
            </button>
          </div>
        </div>

        {/* Target File Size Option */}
        <div className="flex flex-col gap-2.5 p-3.5 rounded-xl bg-zinc-900/40 border border-white/5">
          <div className="flex items-center justify-between">
            <label className="flex items-center gap-2 text-xs font-medium text-zinc-200 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={useTargetSize}
                onChange={(e) => setUseTargetSize(e.target.checked)}
                className="w-4 h-4 rounded border-white/20 accent-orange-500 cursor-pointer"
              />
              <span className="flex items-center gap-1.5">
                <Target size={14} className="text-orange-400" />
                Compress to target file size
              </span>
            </label>
            {useTargetSize && (
              <span className="text-[11px] font-mono text-orange-400 font-semibold">
                {targetSizeKb >= 1024
                  ? `${(targetSizeKb / 1024).toFixed(2)} MB`
                  : `${targetSizeKb} KB`}
              </span>
            )}
          </div>

          {useTargetSize && (
            <div className="flex items-center gap-4 mt-2">
              <input
                type="range"
                min={20}
                max={fileSize ? Math.round(fileSize / 1024) : 5000}
                value={targetSizeKb}
                onChange={(e) => setTargetSizeKb(parseInt(e.target.value) || 50)}
                className="flex-1 accent-orange-500 cursor-pointer"
              />
              <div className="flex items-center gap-1.5">
                <input
                  type="number"
                  min={10}
                  value={targetSizeKb}
                  onChange={(e) => setTargetSizeKb(Math.max(10, parseInt(e.target.value) || 10))}
                  className="w-20 px-2 py-1 rounded bg-zinc-950 border border-white/10 text-right font-mono text-xs text-zinc-200 focus:outline-none focus:border-orange-500"
                />
                <span className="text-xs text-zinc-400">KB</span>
              </div>
            </div>
          )}
        </div>

        {/* Live Preview Display */}
        <div className="flex-1 min-h-[160px] rounded-xl overflow-hidden bg-zinc-950/70 border border-white/5 flex items-center justify-center p-3 relative">
          {assetUrl ? (
            <img
              src={assetUrl}
              alt="Preview"
              className="max-h-[220px] max-w-full object-contain rounded shadow"
            />
          ) : (
            <div className="flex flex-col items-center gap-2 text-zinc-600">
              <FileText size={32} />
              <span className="text-xs">No image preview available</span>
            </div>
          )}
        </div>
      </div>
    </ToolWindowLayout>
  );
}
