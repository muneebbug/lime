import { useState, useEffect, useRef } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Sparkles,
  DownloadCloud,
  CheckCircle2,
  Trash2,
  SplitSquareVertical,
} from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";
import { SegmentedControl, WheelButton } from "../../ui/WheelUI";

export interface RemoveBgToolProps {
  filePath: string;
}

interface ModelStatus {
  installed: boolean;
  path: string;
  file_size: number;
  expected_size: number;
}

export function RemoveBgTool({ filePath }: RemoveBgToolProps) {
  const [modelStatus, setModelStatus] = useState<ModelStatus | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<{
    percent: number;
    downloaded: number;
    total: number;
  }>({ percent: 0, downloaded: 0, total: 0 });

  const [featherRadius, setFeatherRadius] = useState<number>(1);
  const [bgMode, setBgMode] = useState<"transparent" | "white" | "black">("transparent");
  const [format, setFormat] = useState<"png" | "webp">("png");

  const [splitPos, setSplitPos] = useState<number>(50); // 0 to 100%
  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);
  const [processedPreview, setProcessedPreview] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const isDraggingSplit = useRef(false);

  const assetUrl = convertFileSrc(filePath);
  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  // Check model status on mount
  useEffect(() => {
    invoke<ModelStatus>("get_rmbg_model_status")
      .then(setModelStatus)
      .catch((e) => console.error("Failed to get model status", e));

    const unlisten = listen<{
      percent: number;
      downloaded: number;
      total: number;
    }>("model-download-progress", (event) => {
      setDownloadProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleDownloadModel = async () => {
    if (isDownloading) return;
    setIsDownloading(true);
    try {
      await invoke("download_rmbg_model");
      const updated = await invoke<ModelStatus>("get_rmbg_model_status");
      setModelStatus(updated);
    } catch (e) {
      console.error("Failed to download model", e);
      alert(`Model download failed: ${e}`);
    } finally {
      setIsDownloading(false);
    }
  };

  const handleDeleteModel = async () => {
    try {
      await invoke("delete_rmbg_model");
      const updated = await invoke<ModelStatus>("get_rmbg_model_status");
      setModelStatus(updated);
    } catch (e) {
      console.error("Failed to delete model", e);
    }
  };

  const handleSplitMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    isDraggingSplit.current = true;
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isDraggingSplit.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const pos = ((e.clientX - rect.left) / rect.width) * 100;
      setSplitPos(Math.max(5, Math.min(95, pos)));
    };

    const handleMouseUp = () => {
      isDraggingSplit.current = false;
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, []);

  const handleReset = () => {
    setFeatherRadius(1);
    setBgMode("transparent");
    setFormat("png");
    setSplitPos(50);
    setSuccessPath(null);
    setProcessedPreview(null);
  };

  const handleApply = async () => {
    if (isProcessing) return;
    setIsProcessing(true);
    try {
      const bgColor: [number, number, number, number] | null =
        bgMode === "white"
          ? [255, 255, 255, 255]
          : bgMode === "black"
          ? [0, 0, 0, 255]
          : null;

      const res = await invoke<string>("remove_background_file", {
        inputPath: filePath,
        featherRadius,
        bgColor,
        format,
        outputPath: null,
      });

      setSuccessPath(res);
      setProcessedPreview(convertFileSrc(res));
    } catch (e) {
      console.error("Remove BG failed:", e);
      alert(`Remove BG failed: ${e}`);
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <ToolWindowLayout
      title="Remove Background"
      subtitle={fileName}
      icon={<Sparkles size={16} />}
      primaryActionLabel="Export without Background"
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleReset}
      onPrimaryAction={handleApply}
    >
      <div className="flex flex-col h-full gap-3 p-3">
        {/* Model status / download banner */}
        {modelStatus && !modelStatus.installed && (
          <div className="flex items-center justify-between p-3 rounded-xl bg-[#242424] border border-white/[0.08] text-xs">
            <div className="flex items-center gap-2.5">
              <DownloadCloud size={18} className="text-[#ff6339] shrink-0" />
              <div>
                <div className="text-neutral-200 font-semibold">
                  RMBG-1.4 Neural Model (~176 MB)
                </div>
                <div className="text-neutral-400 text-[11px]">
                  Download for high-precision local AI subject isolation.
                </div>
              </div>
            </div>

            <div className="flex items-center gap-2">
              {isDownloading ? (
                <div className="flex items-center gap-2 bg-[#1c1c1c] px-3 py-1.5 rounded-lg border border-white/[0.08]">
                  <div className="w-24 h-1.5 bg-[#333333] rounded-full overflow-hidden">
                    <div
                      style={{ width: `${Math.round(downloadProgress.percent * 100)}%` }}
                      className="h-full bg-[#ff6339] transition-all duration-200"
                    />
                  </div>
                  <span className="text-[11px] font-mono text-[#ff6339]">
                    {Math.round(downloadProgress.percent * 100)}%
                  </span>
                </div>
              ) : (
                <WheelButton
                  variant="primary"
                  onClick={handleDownloadModel}
                >
                  Download Model
                </WheelButton>
              )}
            </div>
          </div>
        )}

        {modelStatus?.installed && (
          <div className="flex items-center justify-between px-3.5 py-2 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-xs">
            <div className="flex items-center gap-2 text-emerald-400 font-medium">
              <CheckCircle2 size={15} />
              <span>RMBG-1.4 AI Model Active (176 MB, local inference)</span>
            </div>
            <button
              onClick={handleDeleteModel}
              className="text-neutral-500 hover:text-red-400 p-1 rounded transition-colors cursor-default"
              title="Remove model to free disk space"
            >
              <Trash2 size={13} />
            </button>
          </div>
        )}

        {/* Controls Toolbar */}
        <div className="flex items-center justify-between gap-4 p-3 rounded-xl bg-[#242424] border border-white/[0.06] text-xs shrink-0">
          {/* Feathering slider */}
          <div className="flex items-center gap-2.5">
            <span className="text-[11px] text-neutral-400 font-medium whitespace-nowrap">
              Feather:
            </span>
            <input
              type="range"
              min={0}
              max={8}
              value={featherRadius}
              onChange={(e) => setFeatherRadius(parseInt(e.target.value) || 0)}
              className="accent-[#ff6339] cursor-default h-1.5 w-24 bg-[#1c1c1c] rounded"
            />
            <span className="font-mono text-[11px] text-neutral-200 w-5 text-right">
              {featherRadius}px
            </span>
          </div>

          {/* Background Replacement Mode */}
          <div className="flex items-center gap-2">
            <span className="text-[11px] text-neutral-400 font-medium">Backdrop:</span>
            <SegmentedControl
              value={bgMode}
              options={[
                { label: "Alpha", value: "transparent" },
                { label: "White", value: "white" },
                { label: "Black", value: "black" },
              ]}
              onChange={(val) => setBgMode(val as any)}
            />
          </div>

          {/* Format selector */}
          <div className="flex items-center gap-2">
            <span className="text-[11px] text-neutral-400 font-medium">Format:</span>
            <SegmentedControl
              value={format}
              options={[
                { label: "PNG", value: "png" },
                { label: "WEBP", value: "webp" },
              ]}
              onChange={(val) => setFormat(val as any)}
            />
          </div>
        </div>

        {/* Live Split Comparison Preview */}
        <div
          ref={containerRef}
          className="relative flex-1 min-h-[300px] rounded-xl overflow-hidden bg-[linear-gradient(45deg,#18181b_25%,transparent_25%),linear-gradient(-45deg,#18181b_25%,transparent_25%),linear-gradient(45deg,transparent_75%,#18181b_75%),linear-gradient(-45deg,transparent_75%,#18181b_75%)] bg-[size:16px_16px] bg-zinc-900 border border-white/10 flex items-center justify-center p-6 select-none"
        >
          {/* Base / Result container */}
          <div className="relative max-h-[360px] max-w-full inline-block">
            {/* Right side: Processed (transparent or solid) */}
            <img
              src={processedPreview ?? assetUrl}
              alt="Processed"
              className="max-h-[360px] max-w-full object-contain pointer-events-none rounded shadow-lg"
            />

            {/* Left side: Original image clipped to splitPos */}
            <div
              style={{ clipPath: `inset(0 ${100 - splitPos}% 0 0)` }}
              className="absolute inset-0"
            >
              <img
                src={assetUrl}
                alt="Original"
                className="max-h-[360px] max-w-full object-contain pointer-events-none rounded shadow-lg"
              />
              <span className="absolute top-2 left-2 px-2 py-0.5 rounded bg-black/60 backdrop-blur-md text-[10px] text-zinc-300 font-mono">
                Original
              </span>
            </div>

            <span className="absolute top-2 right-2 px-2 py-0.5 rounded bg-black/60 backdrop-blur-md text-[10px] text-orange-400 font-mono">
              Result
            </span>

            {/* Draggable Split Handle Line */}
            <div
              onMouseDown={handleSplitMouseDown}
              style={{ left: `${splitPos}%` }}
              className="absolute top-0 bottom-0 w-1 bg-white cursor-ew-resize shadow-[0_0_10px_rgba(0,0,0,0.8)] -translate-x-1/2 flex items-center justify-center"
            >
              <div className="w-6 h-6 rounded-full bg-white text-zinc-950 flex items-center justify-center shadow-lg border border-zinc-300">
                <SplitSquareVertical size={13} />
              </div>
            </div>
          </div>
        </div>
      </div>
    </ToolWindowLayout>
  );
}
