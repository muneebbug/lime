import React, { useMemo } from "react";
import { ActionManifest, WheelPage } from "../../store/wheelStore";

export interface PetalData {
  id: string;
  action: string;
  title: string;
  d: string;
  labelX: number;
  labelY: number;
  subtitle: string;
  icon?: {
    x: number;
    y: number;
    width: number;
    height: number;
    type: "compress" | "metadata" | "edit" | "addbg" | "crop" | "redact";
  };
}

export const CONVERT_PETALS: PetalData[] = [
  {
    id: "convert.png",
    action: "png",
    title: "PNG",
    d: "M 19.2707 -51.7498 L 36.1680 -92.5434 Q 43.8842 -111.1720 26.7781 -116.4816 A 119.52000000000001 119.52000000000001 0 0 0 -26.7781 -116.4816 Q -43.8842 -111.1720 -36.1680 -92.5434 L -19.2707 -51.7498 Q -16.9004 -46.0275 -10.8521 -47.4139 A 48.64000000000001 48.64000000000001 0 0 1 10.8521 -47.4139 Q 16.9004 -46.0275 19.2707 -51.7498 Z",
    labelX: 0,
    labelY: -84.5312,
    subtitle: "Convert to PNG",
  },
  {
    id: "convert.webp",
    action: "webp",
    title: "WEBP",
    d: "M 50.2191 -22.9662 L 91.0127 -39.8635 Q 109.6413 -47.5797 101.2999 -63.4300 A 119.52000000000001 119.52000000000001 0 0 0 63.4300 -101.2999 Q 47.5797 -109.6413 39.8635 -91.0127 L 22.9662 -50.2191 Q 20.5960 -44.4968 25.8531 -41.2003 A 48.64000000000001 48.64000000000001 0 0 1 41.2003 -25.8531 Q 44.4968 -20.5960 50.2191 -22.9662 Z",
    labelX: 59.7726,
    labelY: -59.7726,
    subtitle: "Convert to WEBP",
  },
  {
    id: "convert.heic",
    action: "heic",
    title: "HEIC",
    d: "M 51.7498 19.2707 L 92.5434 36.1680 Q 111.1720 43.8842 116.4816 26.7781 A 119.52000000000001 119.52000000000001 0 0 0 116.4816 -26.7781 Q 111.1720 -43.8842 92.5434 -36.1680 L 51.7498 -19.2707 Q 46.0275 -16.9004 47.4139 -10.8521 A 48.64000000000001 48.64000000000001 0 0 1 47.4139 10.8521 Q 46.0275 16.9004 51.7498 19.2707 Z",
    labelX: 84.5312,
    labelY: 0,
    subtitle: "Convert to HEIC",
  },
  {
    id: "convert.tiff",
    action: "tiff",
    title: "TIFF",
    d: "M 22.9662 50.2191 L 39.8635 91.0127 Q 47.5797 109.6413 63.4300 101.2999 A 119.52000000000001 119.52000000000001 0 0 0 101.2999 63.4300 Q 109.6413 47.5797 91.0127 39.8635 L 50.2191 22.9662 Q 44.4968 20.5960 41.2003 25.8531 A 48.64000000000001 48.64000000000001 0 0 1 25.8531 41.2003 Q 20.5960 44.4968 22.9662 50.2191 Z",
    labelX: 59.7726,
    labelY: 59.7726,
    subtitle: "Convert to TIFF",
  },
  {
    id: "convert.avif",
    action: "avif",
    title: "AVIF",
    d: "M -19.2707 51.7498 L -36.1680 92.5434 Q -43.8842 111.1720 -26.7781 116.4816 A 119.52000000000001 119.52000000000001 0 0 0 26.7781 116.4816 Q 43.8842 111.1720 36.1680 92.5434 L 19.2707 51.7498 Q 16.9004 46.0275 10.8521 47.4139 A 48.64000000000001 48.64000000000001 0 0 1 -10.8521 47.4139 Q -16.9004 46.0275 -19.2707 51.7498 Z",
    labelX: 0,
    labelY: 84.5312,
    subtitle: "Convert to AVIF",
  },
  {
    id: "convert.bmp",
    action: "bmp",
    title: "BMP",
    d: "M -50.2191 22.9662 L -91.0127 39.8635 Q -109.6413 47.5797 -101.2999 63.4300 A 119.52000000000001 119.52000000000001 0 0 0 -63.4300 101.2999 Q -47.5797 109.6413 -39.8635 91.0127 L -22.9662 50.2191 Q -20.5960 44.4968 -25.8531 41.2003 A 48.64000000000001 48.64000000000001 0 0 1 -41.2003 25.8531 Q -44.4968 20.5960 -50.2191 22.9662 Z",
    labelX: -59.7726,
    labelY: 59.7726,
    subtitle: "Convert to BMP",
  },
  {
    id: "convert.pdf",
    action: "pdf",
    title: "PDF",
    d: "M -51.7498 -19.2707 L -92.5434 -36.1680 Q -111.1720 -43.8842 -116.4816 -26.7781 A 119.52000000000001 119.52000000000001 0 0 0 -116.4816 26.7781 Q -111.1720 43.8842 -92.5434 36.1680 L -51.7498 19.2707 Q -46.0275 16.9004 -47.4139 10.8521 A 48.64000000000001 48.64000000000001 0 0 1 -47.4139 -10.8521 Q -46.0275 -16.9004 -51.7498 -19.2707 Z",
    labelX: -84.5312,
    labelY: 0,
    subtitle: "Convert to PDF",
  },
  {
    id: "convert.docx",
    action: "docx",
    title: "DOCX",
    d: "M -22.9662 -50.2191 L -39.8635 -91.0127 Q -47.5797 -109.6413 -63.4300 -101.2999 A 119.52000000000001 119.52000000000001 0 0 0 -101.2999 -63.4300 Q -109.6413 -47.5797 -91.0127 -39.8635 L -50.2191 -22.9662 Q -44.4968 -20.5960 -41.2003 -25.8531 A 48.64000000000001 48.64000000000001 0 0 1 -25.8531 -41.2003 Q -20.5960 -44.4968 -22.9662 -50.2191 Z",
    labelX: -59.7726,
    labelY: -59.7726,
    subtitle: "Convert to DOCX",
  },
];

export const TOOLS_PETALS: PetalData[] = [
  {
    id: "tool.compress",
    action: "compress",
    title: "COMPRESS",
    d: "M 27.2093 -51.1279 L 47.9379 -87.0309 Q 58.0196 -104.4929 41.7529 -111.9899 A 119.52000000000001 119.52000000000001 0 0 0 -41.7529 -111.9899 Q -58.0196 -104.4929 -47.9379 -87.0309 L -27.2093 -51.1279 Q -22.9392 -43.7319 -14.7815 -46.3396 A 48.64000000000001 48.64000000000001 0 0 1 14.7815 -46.3396 Q 22.9392 -43.7319 27.2093 -51.1279 Z",
    icon: { x: -9, y: -101.0312, width: 18, height: 18, type: "compress" },
    labelX: 0,
    labelY: -74.0312,
    subtitle: "Compress Image",
  },
  {
    id: "tool.metadata",
    action: "removeMetadata",
    title: "METADATA",
    d: "M 57.8827 -2.0000 L 99.3399 -2.0000 Q 119.5033 -2.0000 117.8625 -19.8359 A 119.52000000000001 119.52000000000001 0 0 0 76.1096 -92.1540 Q 61.4837 -102.4929 51.4020 -85.0309 L 30.6734 -49.1279 Q 26.4033 -41.7319 32.7405 -35.9710 A 48.64000000000001 48.64000000000001 0 0 1 47.5220 -10.3686 Q 49.3425 -2.0000 57.8827 -2.0000 Z",
    icon: { x: 64.206, y: -58.7656, width: 18, height: 18, type: "metadata" },
    labelX: 73.206,
    labelY: -31.7656,
    subtitle: "View & Clean Metadata",
  },
  {
    id: "tool.edit",
    action: "editImage",
    title: "EDIT",
    d: "M 30.6734 49.1279 L 51.4020 85.0309 Q 61.4837 102.4929 76.1096 92.1540 A 119.52000000000001 119.52000000000001 0 0 0 117.8625 19.8359 Q 119.5033 2.0000 99.3399 2.0000 L 57.8827 2.0000 Q 49.3425 2.0000 47.5220 10.3686 A 48.64000000000001 48.64000000000001 0 0 1 32.7405 35.9710 Q 26.4033 41.7319 30.6734 49.1279 Z",
    icon: { x: 64.206, y: 25.7656, width: 18, height: 18, type: "edit" },
    labelX: 73.206,
    labelY: 52.7656,
    subtitle: "Edit Photo",
  },
  {
    id: "tool.addbg",
    action: "frameImage",
    title: "ADD BG",
    d: "M -27.2093 51.1279 L -47.9379 87.0309 Q -58.0196 104.4929 -41.7529 111.9899 A 119.52000000000001 119.52000000000001 0 0 0 41.7529 111.9899 Q 58.0196 104.4929 47.9379 87.0309 L 27.2093 51.1279 Q 22.9392 43.7319 14.7815 46.3396 A 48.64000000000001 48.64000000000001 0 0 1 -14.7815 46.3396 Q -22.9392 43.7319 -27.2093 51.1279 Z",
    icon: { x: -9, y: 68.0312, width: 18, height: 18, type: "addbg" },
    labelX: 0,
    labelY: 95.0312,
    subtitle: "Add Background",
  },
  {
    id: "tool.crop",
    action: "cropImage",
    title: "CROP",
    d: "M -57.8827 2.0000 L -99.3399 2.0000 Q -119.5033 2.0000 -117.8625 19.8359 A 119.52000000000001 119.52000000000001 0 0 0 -76.1096 92.1540 Q -61.4837 102.4929 -51.4020 85.0309 L -30.6734 49.1279 Q -26.4033 41.7319 -32.7405 35.9710 A 48.64000000000001 48.64000000000001 0 0 1 -47.5220 10.3686 Q -49.3425 2.0000 -57.8827 2.0000 Z",
    icon: { x: -82.206, y: 25.7656, width: 18, height: 18, type: "crop" },
    labelX: -73.206,
    labelY: 52.7656,
    subtitle: "Crop Image",
  },
  {
    id: "tool.redact",
    action: "redactImage",
    title: "REDACT",
    d: "M -30.6734 -49.1279 L -51.4020 -85.0309 Q -61.4837 -102.4929 -76.1096 -92.1540 A 119.52000000000001 119.52000000000001 0 0 0 -117.8625 -19.8359 Q -119.5033 -2.0000 -99.3399 -2.0000 L -57.8827 -2.0000 Q -49.3425 -2.0000 -47.5220 -10.3686 A 48.64000000000001 48.64000000000001 0 0 1 -32.7405 -35.9710 Q -26.4033 -41.7319 -30.6734 -49.1279 Z",
    icon: { x: -82.206, y: -58.7656, width: 18, height: 18, type: "redact" },
    labelX: -73.206,
    labelY: -31.7656,
    subtitle: "Redact Photo",
  },
];

export function hitTestWedge(
  cursorX: number,
  cursorY: number,
  pageOrCount: WheelPage | number,
  wheelCenterX = 200,
  wheelCenterY = 200,
  scaleOrOuter = 1,
  _innerRadius?: number
): number | null {
  const isCount = typeof pageOrCount === "number";
  const count = isCount ? pageOrCount : pageOrCount === "convert" ? 8 : 6;
  const scale =
    isCount && _innerRadius !== undefined
      ? scaleOrOuter / 148
      : typeof scaleOrOuter === "number"
      ? scaleOrOuter
      : 1;

  const dx = (cursorX - wheelCenterX) / (scale || 1);
  const dy = (cursorY - wheelCenterY) / (scale || 1);
  const dist = Math.sqrt(dx * dx + dy * dy);

  // Inner petal boundary is 48.64, outer boundary is 119.52
  if (dist < 44 || dist > 125) return null;

  const angle = Math.atan2(dy, dx);

  if (count === 8) {
    let normalized = (angle + Math.PI / 2 + Math.PI / 8) % (2 * Math.PI);
    if (normalized < 0) normalized += 2 * Math.PI;
    const index = Math.floor(normalized / (Math.PI / 4));
    return index >= 0 && index < 8 ? index : null;
  } else {
    let normalized = (angle + Math.PI / 2 + Math.PI / 6) % (2 * Math.PI);
    if (normalized < 0) normalized += 2 * Math.PI;
    const index = Math.floor(normalized / (Math.PI / 3));
    return index >= 0 && index < 6 ? index : null;
  }
}

export function filterActions(
  actions: ActionManifest[],
  page: WheelPage,
  _extensions: string[],
  _contextFilterEnabled = true
): ActionManifest[] {
  const petals = page === "convert" ? CONVERT_PETALS : TOOLS_PETALS;
  return petals.map((p, idx) => {
    const existing = actions.find((a) => a.id === p.id);
    if (existing) return existing;
    return {
      id: p.id,
      title: p.title,
      icon: p.id,
      category: page === "convert" ? "convert" : "tools",
      accepts: { extensions: [], multi: true },
      kind: page === "convert" ? "instant" : "window",
      enabled: true,
      order: idx,
    } as ActionManifest;
  });
}

function playHoverTick() {
  try {
    const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
    if (!AudioCtx) return;
    const ctx = new AudioCtx();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(1200, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(350, ctx.currentTime + 0.015);
    gain.gain.setValueAtTime(0.04, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.015);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.02);
  } catch {}
}

function ToolIcon({ type }: { type: string }) {
  switch (type) {
    case "compress":
      return (
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M4 14h6v6M10 14L3 21M20 10h-6V4M14 10l7-7" />
        </svg>
      );
    case "metadata":
      return (
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M9.88 9.88a3 3 0 1 0 4.24 4.24" />
          <path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" />
          <path d="M6.61 6.61A13.526 13.526 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" />
          <line x1="2" x2="22" y1="2" y2="22" />
        </svg>
      );
    case "edit":
      return (
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <line x1="4" y1="21" x2="4" y2="14" />
          <line x1="4" y1="10" x2="4" y2="3" />
          <line x1="12" y1="21" x2="12" y2="12" />
          <line x1="12" y1="8" x2="12" y2="3" />
          <line x1="20" y1="21" x2="20" y2="16" />
          <line x1="20" y1="12" x2="20" y2="3" />
          <line x1="1" y1="14" x2="7" y2="14" />
          <line x1="9" y1="8" x2="15" y2="8" />
          <line x1="17" y1="16" x2="23" y2="16" />
        </svg>
      );
    case "addbg":
      return (
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
          <circle cx="8.5" cy="8.5" r="1.5" />
          <polyline points="21 15 16 10 5 21" />
        </svg>
      );
    case "crop":
      return (
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M6 2v14a2 2 0 0 0 2 2h14" />
          <path d="M18 22V8a2 2 0 0 0-2-2H2" />
        </svg>
      );
    case "redact":
      return (
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M9.88 9.88a3 3 0 1 0 4.24 4.24" />
          <path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" />
          <path d="M6.61 6.61A13.526 13.526 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" />
          <line x1="2" x2="22" y1="2" y2="22" />
        </svg>
      );
    default:
      return null;
  }
}

interface RadialWheelProps {
  files: string[];
  extensions: string[];
  currentPage: WheelPage;
  hoveredWedge: string | null;
  onTogglePage: () => void;
  onWedgeHover: (id: string | null) => void;
  onWedgeDrop: (actionId: string, files: string[]) => void;
  size?: number;
  contextFilterEnabled?: boolean;
  soundEnabled?: boolean;
}

function RadialWheelInner({
  files,
  currentPage,
  hoveredWedge,
  onTogglePage,
  onWedgeHover,
  onWedgeDrop,
  size = 272,
  soundEnabled = false,
}: RadialWheelProps) {
  const activePetals = useMemo(
    () => (currentPage === "convert" ? CONVERT_PETALS : TOOLS_PETALS),
    [currentPage]
  );

  const hoveredPetal = useMemo(
    () => activePetals.find((p) => p.id === hoveredWedge) ?? null,
    [activePetals, hoveredWedge]
  );

  const wheelSize = size || 272;
  const scale = wheelSize / 272;

  return (
    <div className="flex flex-col items-center justify-center select-none pointer-events-auto">
      {/* Top Shortcut Indicator + Category Title */}
      <div className="flex flex-col items-center gap-1.5 mb-2 pointer-events-none select-none">
        {currentPage === "convert" ? (
          <div className="flex flex-col items-center justify-center w-11 h-12 bg-white/95 border border-white/80 rounded-xl shadow-[0_4px_12px_rgba(0,0,0,0.08),0_1px_2px_rgba(0,0,0,0.04),inset_0_-2px_0_rgba(0,0,0,0.1)] text-neutral-700">
            <span className="text-sm font-bold leading-none">⇧</span>
            <span className="text-[10px] font-medium text-neutral-500 mt-1 uppercase tracking-wider">
              shift
            </span>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <div className="flex flex-col items-center justify-center w-11 h-12 bg-white/95 border border-white/80 rounded-xl shadow-[0_4px_12px_rgba(0,0,0,0.08),0_1px_2px_rgba(0,0,0,0.04),inset_0_-2px_0_rgba(0,0,0,0.1)] text-neutral-700">
              <span className="text-sm font-bold leading-none">⇧</span>
              <span className="text-[10px] font-medium text-neutral-500 mt-1 uppercase tracking-wider">
                shift
              </span>
            </div>
            <span className="text-neutral-400 font-bold text-xs">+</span>
            <div className="flex flex-col items-center justify-center w-12 h-12 bg-white/95 border border-white/80 rounded-xl shadow-[0_4px_12px_rgba(0,0,0,0.08),0_1px_2px_rgba(0,0,0,0.04),inset_0_-2px_0_rgba(0,0,0,0.1)] text-neutral-700">
              <span className="text-sm font-bold leading-none">⌥</span>
              <span className="text-[10px] font-medium text-neutral-500 mt-1 uppercase tracking-wider">
                option
              </span>
            </div>
          </div>
        )}
        <span className="text-xs font-semibold text-neutral-700/90 tracking-wide">
          {currentPage === "convert" ? "Convert formats" : "Advanced tools"}
        </span>
      </div>

      {/* Main Circular Radial Wheel SVG */}
      <div
        className="relative"
        style={{ width: wheelSize, height: wheelSize }}
        onMouseMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const mx = e.clientX - rect.left;
          const my = e.clientY - rect.top;
          const idx = hitTestWedge(
            mx,
            my,
            currentPage,
            wheelSize / 2,
            wheelSize / 2,
            scale
          );
          const nextId = idx !== null ? activePetals[idx]?.id ?? null : null;
          if (nextId !== hoveredWedge) {
            if (soundEnabled && nextId !== null) {
              playHoverTick();
            }
            onWedgeHover(nextId);
          }
        }}
        onMouseLeave={() => {
          if (hoveredWedge !== null) {
            onWedgeHover(null);
          }
        }}
        onMouseUp={() => {
          if (hoveredWedge && files.length > 0) {
            onWedgeDrop(hoveredWedge, files);
          }
        }}
      >
        <svg
          viewBox="-136 -136 272 272"
          width={wheelSize}
          height={wheelSize}
          className="overflow-visible"
          role="group"
          aria-label={
            currentPage === "convert"
              ? "Choose a format to preview"
              : "Choose a tool to preview"
          }
        >
          <defs>
            {/* Ambient petal frosted glass fill */}
            <radialGradient
              id="conversion-fan-glass"
              cx="0"
              cy="0"
              r="120"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0" stopColor="#ffffff" stopOpacity="0.88" />
              <stop offset="0.68" stopColor="#f3f6f7" stopOpacity="0.65" />
              <stop offset="1" stopColor="#f4f6f6" stopOpacity="0.82" />
            </radialGradient>
            <radialGradient
              id="advanced-fan-glass"
              cx="0"
              cy="0"
              r="120"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0" stopColor="#ffffff" stopOpacity="0.88" />
              <stop offset="0.68" stopColor="#f3f6f7" stopOpacity="0.65" />
              <stop offset="1" stopColor="#f4f6f6" stopOpacity="0.82" />
            </radialGradient>

            {/* Active highlighted petal gradient */}
            <linearGradient
              id="petal-active-gradient"
              x1="0%"
              y1="0%"
              x2="100%"
              y2="100%"
            >
              <stop offset="0%" stopColor="#ff5d1c" />
              <stop offset="100%" stopColor="#f24500" />
            </linearGradient>

            {/* Shadows */}
            <filter id="center-pill-shadow" x="-30%" y="-30%" width="160%" height="160%">
              <feDropShadow
                dx="0"
                dy="4"
                stdDeviation="8"
                floodColor="#000000"
                floodOpacity="0.1"
              />
              <feDropShadow
                dx="0"
                dy="1"
                stdDeviation="2"
                floodColor="#000000"
                floodOpacity="0.05"
              />
            </filter>
            <filter id="wheel-outer-shadow" x="-30%" y="-30%" width="160%" height="160%">
              <feDropShadow
                dx="0"
                dy="10"
                stdDeviation="24"
                floodColor="#000000"
                floodOpacity="0.14"
              />
            </filter>
          </defs>

          {/* Frosted translucent circular glass backing disk */}
          <circle
            cx="0"
            cy="0"
            r="126"
            fill="rgba(244, 247, 249, 0.45)"
            stroke="rgba(255, 255, 255, 0.8)"
            strokeWidth="1.5"
            filter="url(#wheel-outer-shadow)"
          />

          {/* Individual Petal Wedges */}
          {activePetals.map((petal) => {
            const isHighlighted = hoveredWedge === petal.id;
            return (
              <g
                key={petal.id}
                className={`preview-target ${isHighlighted ? "is-highlighted" : ""}`}
                data-action={petal.action}
                role="button"
                aria-label={`Preview ${petal.title}`}
                aria-pressed={isHighlighted}
                style={{ cursor: "pointer" }}
                onMouseEnter={() => {
                  if (soundEnabled && hoveredWedge !== petal.id) {
                    playHoverTick();
                  }
                  onWedgeHover(petal.id);
                }}
                onClick={() => {
                  if (files.length > 0) {
                    onWedgeDrop(petal.id, files);
                  }
                }}
              >
                <path
                  d={petal.d}
                  className="preview-petal"
                  fill={
                    isHighlighted
                      ? "url(#petal-active-gradient)"
                      : "url(#conversion-fan-glass)"
                  }
                  stroke={
                    isHighlighted ? "#ff7538" : "rgba(255, 255, 255, 0.85)"
                  }
                  strokeWidth={isHighlighted ? 1.5 : 1.2}
                  style={{
                    filter: isHighlighted
                      ? "drop-shadow(0 4px 14px rgba(255, 84, 25, 0.45))"
                      : "drop-shadow(0 2px 5px rgba(0,0,0,0.06))",
                    transformOrigin: "0px 0px",
                    transform: isHighlighted ? "scale(1.025)" : "scale(1)",
                    transition:
                      "transform 0.12s cubic-bezier(0.16, 1, 0.3, 1), fill 0.12s ease, stroke 0.12s ease",
                  }}
                />

                {petal.icon && (
                  <g
                    transform={`translate(${petal.icon.x + 1}, ${petal.icon.y + 1})`}
                    className="wheel-tool-icon preview-tool-icon"
                    style={{
                      color: isHighlighted ? "#ffffff" : "#222428",
                      transformOrigin: `${petal.icon.x + 9}px ${petal.icon.y + 9}px`,
                      transform: isHighlighted ? "scale(1.025)" : "scale(1)",
                      transition: "color 0.12s ease, transform 0.12s ease",
                      pointerEvents: "none",
                    }}
                  >
                    <ToolIcon type={petal.icon.type} />
                  </g>
                )}

                <text
                  className="wheel-label preview-format-label preview-tool-label wheel-tool-label"
                  x={petal.labelX}
                  y={petal.labelY}
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill={isHighlighted ? "#ffffff" : "#222428"}
                  fontSize={petal.icon ? "9.5px" : "11px"}
                  fontWeight={isHighlighted ? "800" : "700"}
                  fontFamily="system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
                  letterSpacing="0.04em"
                  style={{
                    pointerEvents: "none",
                    userSelect: "none",
                    transformOrigin: "0px 0px",
                    transform: isHighlighted ? "scale(1.025)" : "scale(1)",
                    transition: "fill 0.12s ease, transform 0.12s ease",
                  }}
                >
                  {petal.title}
                </text>
              </g>
            );
          })}

          {/* Central Pill / Badge */}
          <g
            className="center-target"
            style={{ cursor: "pointer" }}
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onTogglePage();
            }}
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onTogglePage();
            }}
            onWheel={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onTogglePage();
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onTogglePage();
            }}
          >
            <circle
              cx="0"
              cy="0"
              r="41"
              fill="#ffffff"
              stroke="rgba(0,0,0,0.05)"
              strokeWidth="1"
              filter="url(#center-pill-shadow)"
            />

            {hoveredPetal ? (
              <text
                x="0"
                y="0"
                textAnchor="middle"
                dominantBaseline="central"
                fill="#1e2022"
                fontSize="13px"
                fontWeight="700"
                fontFamily="system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
                letterSpacing="0.02em"
                style={{ pointerEvents: "none", userSelect: "none" }}
              >
                {hoveredPetal.title}
              </text>
            ) : (
              <>
                <text
                  x="0"
                  y="-4"
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill="#2d3135"
                  fontSize="10px"
                  fontWeight="700"
                  fontFamily="system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
                  letterSpacing="0.06em"
                  style={{ pointerEvents: "none", userSelect: "none" }}
                >
                  {currentPage === "convert" ? "CONVERT" : "TOOLS"}
                </text>
                {/* 2 Page Dots */}
                <circle
                  cx="-4"
                  cy="11"
                  r="2.2"
                  fill={currentPage === "convert" ? "#ff5419" : "#d1d5db"}
                />
                <circle
                  cx="4"
                  cy="11"
                  r="2.2"
                  fill={currentPage === "tools" ? "#ff5419" : "#d1d5db"}
                />
              </>
            )}
          </g>
        </svg>
      </div>

      {/* Action Subtitle Below the Wheel */}
      <div className="h-6 flex items-center justify-center mt-2 pointer-events-none select-none">
        <span className="text-xs font-medium text-neutral-600 tracking-wide transition-opacity duration-150">
          {hoveredPetal
            ? hoveredPetal.subtitle
            : currentPage === "convert"
            ? "Convert formats"
            : "Advanced tools"}
        </span>
      </div>
    </div>
  );
}

export const RadialWheel = React.memo(RadialWheelInner);
