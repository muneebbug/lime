import React, { useState, useRef, useEffect, useCallback } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import {
  PenTool,
  Highlighter,
  ArrowUpRight,
  Square,
  Type,
  ListOrdered,
  Undo2,
  Redo2,
  Trash2,
  Sparkles,
  Check,
} from "lucide-react";
import { ToolWindowLayout } from "./ToolWindowLayout";

export interface AnnotateToolProps {
  filePath: string;
}

export type ToolType = "pen" | "highlighter" | "arrow" | "rectangle" | "text" | "step";

interface AnnotationAction {
  type: ToolType;
  color: string;
  lineWidth: number;
  points?: { x: number; y: number }[]; // For pen & highlighter
  start?: { x: number; y: number }; // For arrow & rect
  end?: { x: number; y: number }; // For arrow & rect
  text?: string;
  stepNumber?: number; // For step markers
}

const COLOR_SWATCHES = [
  { name: "Orange", hex: "#f97316" },
  { name: "Red", hex: "#ef4444" },
  { name: "Amber", hex: "#f59e0b" },
  { name: "Emerald", hex: "#10b981" },
  { name: "Cyan", hex: "#06b6d4" },
  { name: "Blue", hex: "#3b82f6" },
  { name: "Purple", hex: "#a855f7" },
  { name: "White", hex: "#ffffff" },
  { name: "Black", hex: "#000000" },
];

const STROKE_WIDTHS = [
  { label: "Thin", val: 3 },
  { label: "Med", val: 6 },
  { label: "Thick", val: 12 },
];

export function AnnotateTool({ filePath }: AnnotateToolProps) {
  const [naturalSize, setNaturalSize] = useState<{ width: number; height: number } | null>(null);
  const [activeTool, setActiveTool] = useState<ToolType>("arrow");
  const [activeColor, setActiveColor] = useState<string>("#f97316");
  const [strokeWidth, setStrokeWidth] = useState<number>(6);
  const [currentStep, setCurrentStep] = useState<number>(1);

  // Undo / Redo stacks
  const [history, setHistory] = useState<AnnotationAction[]>([]);
  const [redoStack, setRedoStack] = useState<AnnotationAction[]>([]);

  // In-progress drawing action
  const [currentAction, setCurrentAction] = useState<AnnotationAction | null>(null);
  const [textInput, setTextInput] = useState<{ x: number; y: number; text: string } | null>(null);

  const [isProcessing, setIsProcessing] = useState(false);
  const [successPath, setSuccessPath] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const assetUrl = convertFileSrc(filePath);
  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  const handleImageLoad = (e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    setNaturalSize({ width: img.naturalWidth, height: img.naturalHeight });
  };

  // Convert screen coordinates to natural image pixel coordinates
  const getNaturalCoords = useCallback(
    (clientX: number, clientY: number) => {
      const img = imgRef.current;
      if (!img || !naturalSize) return null;

      const rect = img.getBoundingClientRect();
      const clickX = clientX - rect.left;
      const clickY = clientY - rect.top;

      const clampedX = Math.max(0, Math.min(clickX, rect.width));
      const clampedY = Math.max(0, Math.min(clickY, rect.height));

      const scaleX = naturalSize.width / rect.width;
      const scaleY = naturalSize.height / rect.height;

      return {
        x: clampedX * scaleX,
        y: clampedY * scaleY,
      };
    },
    [naturalSize]
  );

  // Render all annotations onto the full-resolution canvas
  const redrawCanvas = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || !naturalSize) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Clear transparent canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    const allActions = [...history];
    if (currentAction) {
      allActions.push(currentAction);
    }

    for (const act of allActions) {
      ctx.save();
      ctx.lineCap = "round";
      ctx.lineJoin = "round";

      if (act.type === "highlighter") {
        ctx.strokeStyle = act.color;
        ctx.globalAlpha = 0.35;
        ctx.lineWidth = act.lineWidth * 3;
      } else {
        ctx.strokeStyle = act.color;
        ctx.fillStyle = act.color;
        ctx.lineWidth = act.lineWidth;
        ctx.globalAlpha = 1.0;
      }

      if (act.type === "pen" || act.type === "highlighter") {
        if (act.points && act.points.length > 0) {
          ctx.beginPath();
          ctx.moveTo(act.points[0].x, act.points[0].y);
          for (let i = 1; i < act.points.length; i++) {
            ctx.lineTo(act.points[i].x, act.points[i].y);
          }
          ctx.stroke();
        }
      } else if (act.type === "rectangle" && act.start && act.end) {
        const x = Math.min(act.start.x, act.end.x);
        const y = Math.min(act.start.y, act.end.y);
        const w = Math.abs(act.end.x - act.start.x);
        const h = Math.abs(act.end.y - act.start.y);

        ctx.beginPath();
        // Rounded rectangle
        const r = Math.min(12, w / 4, h / 4);
        ctx.roundRect(x, y, w, h, r);
        ctx.stroke();
      } else if (act.type === "arrow" && act.start && act.end) {
        const fromX = act.start.x;
        const fromY = act.start.y;
        const toX = act.end.x;
        const toY = act.end.y;

        const headlen = Math.max(16, act.lineWidth * 3.5);
        const angle = Math.atan2(toY - fromY, toX - fromX);

        // Arrow line
        ctx.beginPath();
        ctx.moveTo(fromX, fromY);
        ctx.lineTo(toX, toY);
        ctx.stroke();

        // Arrow head
        ctx.beginPath();
        ctx.moveTo(toX, toY);
        ctx.lineTo(
          toX - headlen * Math.cos(angle - Math.PI / 6),
          toY - headlen * Math.sin(angle - Math.PI / 6)
        );
        ctx.lineTo(
          toX - headlen * Math.cos(angle + Math.PI / 6),
          toY - headlen * Math.sin(angle + Math.PI / 6)
        );
        ctx.closePath();
        ctx.fillStyle = act.color;
        ctx.fill();
      } else if (act.type === "text" && act.start && act.text) {
        const fontSize = Math.max(18, act.lineWidth * 4);
        ctx.font = `bold ${fontSize}px sans-serif`;

        // Background pill
        const metrics = ctx.measureText(act.text);
        const pad = fontSize * 0.3;
        ctx.fillStyle = "rgba(0,0,0,0.75)";
        ctx.beginPath();
        ctx.roundRect(
          act.start.x - pad,
          act.start.y - fontSize - pad / 2,
          metrics.width + pad * 2,
          fontSize + pad * 1.5,
          6
        );
        ctx.fill();

        // Text fill
        ctx.fillStyle = act.color;
        ctx.fillText(act.text, act.start.x, act.start.y);
      } else if (act.type === "step" && act.start && act.stepNumber) {
        const radius = Math.max(18, act.lineWidth * 3.5);

        // Circular background with border & shadow
        ctx.shadowColor = "rgba(0,0,0,0.5)";
        ctx.shadowBlur = 8;
        ctx.beginPath();
        ctx.arc(act.start.x, act.start.y, radius, 0, Math.PI * 2);
        ctx.fillStyle = act.color;
        ctx.fill();
        ctx.shadowBlur = 0;

        ctx.strokeStyle = "#ffffff";
        ctx.lineWidth = Math.max(2, radius * 0.1);
        ctx.stroke();

        // Number inside circle
        ctx.fillStyle = act.color === "#ffffff" ? "#000000" : "#ffffff";
        const fontSize = radius * 1.1;
        ctx.font = `bold ${fontSize}px sans-serif`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(act.stepNumber.toString(), act.start.x, act.start.y);
      }

      ctx.restore();
    }
  }, [history, currentAction, naturalSize]);

  // Redraw when history or size changes
  useEffect(() => {
    redrawCanvas();
  }, [redrawCanvas]);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0 || !naturalSize) return;
    const pt = getNaturalCoords(e.clientX, e.clientY);
    if (!pt) return;

    if (activeTool === "step") {
      const newAction: AnnotationAction = {
        type: "step",
        color: activeColor,
        lineWidth: strokeWidth,
        start: pt,
        stepNumber: currentStep,
      };
      setHistory((prev) => [...prev, newAction]);
      setRedoStack([]);
      setCurrentStep((prev) => prev + 1);
      return;
    }

    if (activeTool === "text") {
      setTextInput({ x: pt.x, y: pt.y, text: "" });
      return;
    }

    if (activeTool === "pen" || activeTool === "highlighter") {
      setCurrentAction({
        type: activeTool,
        color: activeColor,
        lineWidth: strokeWidth,
        points: [pt],
      });
    } else if (activeTool === "arrow" || activeTool === "rectangle") {
      setCurrentAction({
        type: activeTool,
        color: activeColor,
        lineWidth: strokeWidth,
        start: pt,
        end: pt,
      });
    }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!currentAction || !naturalSize) return;
    const pt = getNaturalCoords(e.clientX, e.clientY);
    if (!pt) return;

    if (currentAction.type === "pen" || currentAction.type === "highlighter") {
      setCurrentAction((prev) =>
        prev
          ? {
              ...prev,
              points: [...(prev.points || []), pt],
            }
          : null
      );
    } else if (currentAction.type === "arrow" || currentAction.type === "rectangle") {
      setCurrentAction((prev) =>
        prev
          ? {
              ...prev,
              end: pt,
            }
          : null
      );
    }
  };

  const handleMouseUp = () => {
    if (currentAction) {
      setHistory((prev) => [...prev, currentAction]);
      setRedoStack([]);
      setCurrentAction(null);
    }
  };

  const handleCommitText = () => {
    if (textInput && textInput.text.trim()) {
      const newAction: AnnotationAction = {
        type: "text",
        color: activeColor,
        lineWidth: strokeWidth,
        start: { x: textInput.x, y: textInput.y },
        text: textInput.text.trim(),
      };
      setHistory((prev) => [...prev, newAction]);
      setRedoStack([]);
    }
    setTextInput(null);
  };

  const handleUndo = () => {
    if (history.length === 0) return;
    const last = history[history.length - 1];
    if (last.type === "step" && last.stepNumber) {
      setCurrentStep(last.stepNumber);
    }
    setRedoStack((prev) => [last, ...prev]);
    setHistory((prev) => prev.slice(0, -1));
  };

  const handleRedo = () => {
    if (redoStack.length === 0) return;
    const next = redoStack[0];
    if (next.type === "step" && next.stepNumber) {
      setCurrentStep(next.stepNumber + 1);
    }
    setHistory((prev) => [...prev, next]);
    setRedoStack((prev) => prev.slice(1));
  };

  const handleClear = () => {
    setHistory([]);
    setRedoStack([]);
    setCurrentStep(1);
    setTextInput(null);
  };

  // Keyboard shortcut Ctrl+Z / Ctrl+Y
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "z") {
        e.preventDefault();
        if (e.shiftKey) {
          handleRedo();
        } else {
          handleUndo();
        }
      } else if ((e.ctrlKey || e.metaKey) && e.key === "y") {
        e.preventDefault();
        handleRedo();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [history, redoStack]);

  const handlePrimaryAction = async () => {
    if (history.length === 0 && !currentAction) {
      setErrorMessage("Please draw or add at least one annotation.");
      return;
    }

    const canvas = canvasRef.current;
    if (!canvas) return;

    setIsProcessing(true);
    setErrorMessage(null);

    try {
      const dataUrl = canvas.toDataURL("image/png");
      const out = await invoke<string>("annotate_image_file", {
        inputPath: filePath,
        overlayBase64: dataUrl,
        outputPath: null,
      });

      setSuccessPath(out);
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err.message ?? "Annotation failed");
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <ToolWindowLayout
      title="Annotate"
      subtitle={fileName}
      icon={<PenTool size={16} className="text-orange-500" />}
      primaryActionLabel={history.length > 0 ? `Apply & Save (${history.length})` : "Apply"}
      isProcessing={isProcessing}
      successResultPath={successPath}
      onReset={handleClear}
      onPrimaryAction={handlePrimaryAction}
    >
      <div className="flex flex-col h-full w-full overflow-hidden">
        {/* Top Toolbars: Tools, Colors, Sizes */}
        <div className="flex flex-wrap items-center justify-between gap-3 px-5 py-2.5 bg-[#181818] border-b border-white/[0.06]">
          {/* Tool selectors */}
          <div className="flex items-center gap-0.5 p-0.5 bg-[#242424] rounded-lg border border-white/[0.08]">
            <button
              onClick={() => setActiveTool("arrow")}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeTool === "arrow"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
              title="Arrow"
            >
              <ArrowUpRight size={13} />
              <span>Arrow</span>
            </button>

            <button
              onClick={() => setActiveTool("pen")}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeTool === "pen"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
              title="Pen"
            >
              <PenTool size={13} />
              <span>Pen</span>
            </button>

            <button
              onClick={() => setActiveTool("highlighter")}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeTool === "highlighter"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
              title="Highlighter"
            >
              <Highlighter size={13} />
              <span>Highlighter</span>
            </button>

            <button
              onClick={() => setActiveTool("rectangle")}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeTool === "rectangle"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
              title="Rectangle"
            >
              <Square size={13} />
              <span>Box</span>
            </button>

            <button
              onClick={() => setActiveTool("text")}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeTool === "text"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
              title="Text"
            >
              <Type size={13} />
              <span>Text</span>
            </button>

            <button
              onClick={() => setActiveTool("step")}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-default ${
                activeTool === "step"
                  ? "bg-white/[0.14] text-white shadow-sm font-semibold"
                  : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
              }`}
              title="Numbered Step Markers (1, 2, 3...)"
            >
              <ListOrdered size={13} />
              <span>Step ({currentStep})</span>
            </button>
          </div>

          {/* Color swatches */}
          <div className="flex items-center gap-1.5 px-2 py-1 bg-[#242424] rounded-lg border border-white/[0.08]">
            {COLOR_SWATCHES.map((swatch) => (
              <button
                key={swatch.hex}
                onClick={() => setActiveColor(swatch.hex)}
                style={{ backgroundColor: swatch.hex }}
                className={`w-4 h-4 rounded-full border transition-transform cursor-default flex items-center justify-center ${
                  activeColor === swatch.hex
                    ? "scale-125 border-white shadow-md ring-2 ring-white/20"
                    : "border-white/20 hover:scale-110 opacity-70 hover:opacity-100"
                }`}
                title={swatch.name}
              >
                {activeColor === swatch.hex && (
                  <Check
                    size={9}
                    className={swatch.hex === "#ffffff" ? "text-black" : "text-white"}
                    strokeWidth={3}
                  />
                )}
              </button>
            ))}
          </div>

          {/* Stroke width & History controls */}
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-0.5 bg-[#242424] p-0.5 rounded-lg border border-white/[0.08]">
              {STROKE_WIDTHS.map((sw) => (
                <button
                  key={sw.val}
                  onClick={() => setStrokeWidth(sw.val)}
                  className={`px-2 py-1 rounded-md text-[11px] font-medium transition-colors cursor-default ${
                    strokeWidth === sw.val
                      ? "bg-white/[0.14] text-white font-semibold"
                      : "text-neutral-400 hover:text-neutral-200 hover:bg-white/[0.04]"
                  }`}
                >
                  {sw.label}
                </button>
              ))}
            </div>

            <button
              onClick={handleUndo}
              disabled={history.length === 0}
              className="p-1.5 rounded-lg text-neutral-400 hover:text-white bg-[#242424] hover:bg-[#2c2c2c] border border-white/[0.08] disabled:opacity-40 disabled:pointer-events-none transition-colors cursor-default"
              title="Undo (Ctrl+Z)"
            >
              <Undo2 size={13} />
            </button>

            <button
              onClick={handleRedo}
              disabled={redoStack.length === 0}
              className="p-1.5 rounded-lg text-neutral-400 hover:text-white bg-[#242424] hover:bg-[#2c2c2c] border border-white/[0.08] disabled:opacity-40 disabled:pointer-events-none transition-colors cursor-default"
              title="Redo (Ctrl+Y)"
            >
              <Redo2 size={13} />
            </button>

            <button
              onClick={handleClear}
              disabled={history.length === 0}
              className="p-1.5 rounded-lg text-neutral-400 hover:text-red-400 bg-[#242424] hover:bg-red-500/10 border border-white/[0.08] disabled:opacity-40 disabled:pointer-events-none transition-colors cursor-default"
              title="Clear all"
            >
              <Trash2 size={13} />
            </button>
          </div>
        </div>

        {/* Canvas Display Viewport */}
        <div
          ref={containerRef}
          className="relative flex-1 flex items-center justify-center p-6 select-none overflow-hidden bg-black/40"
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          style={{ cursor: activeTool === "text" ? "text" : "crosshair" }}
        >
          <div className="relative inline-block max-h-full max-w-full">
            <img
              ref={imgRef}
              src={assetUrl}
              alt="To annotate"
              onLoad={handleImageLoad}
              className="max-h-[60vh] max-w-[80vw] object-contain rounded shadow-2xl pointer-events-none"
              draggable={false}
            />

            {/* Natural-resolution overlay canvas aligned with image */}
            {naturalSize && (
              <canvas
                ref={canvasRef}
                width={naturalSize.width}
                height={naturalSize.height}
                className="absolute inset-0 w-full h-full pointer-events-none"
              />
            )}

            {/* In-place Text Entry Modal/Popover */}
            {textInput && naturalSize && (
              <div
                style={{
                  left: `${(textInput.x / naturalSize.width) * 100}%`,
                  top: `${(textInput.y / naturalSize.height) * 100}%`,
                }}
                className="absolute z-20 -translate-y-full mb-1 flex items-center gap-1.5 p-1.5 bg-zinc-900 border border-white/20 rounded-lg shadow-2xl backdrop-blur-md"
              >
                <input
                  type="text"
                  autoFocus
                  placeholder="Type note..."
                  value={textInput.text}
                  onChange={(e) => setTextInput({ ...textInput, text: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleCommitText();
                    } else if (e.key === "Escape") {
                      setTextInput(null);
                    }
                  }}
                  className="px-2 py-1 text-xs bg-black/50 border border-white/10 rounded text-white outline-none focus:border-orange-500 w-44"
                />
                <button
                  onClick={handleCommitText}
                  className="px-2 py-1 bg-orange-500 text-white rounded text-xs font-medium cursor-default hover:bg-orange-600"
                >
                  OK
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Footer Bar */}
        <div className="flex items-center justify-between px-6 py-2.5 bg-zinc-900/60 border-t border-white/5 text-xs text-zinc-400">
          <div className="flex items-center gap-3">
            <span>
              Annotations: <strong className="text-zinc-200">{history.length}</strong>
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
            <span>High-res vector compositing • Full pixel fidelity</span>
          </div>
        </div>
      </div>
    </ToolWindowLayout>
  );
}
