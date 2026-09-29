import React, { useMemo } from "react";
import { useWheelStore, ActionManifest, WheelPage } from "../../store/wheelStore";

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

const DEFAULT_WHEEL_SIZE = 320;
const BASE_INNER_RADIUS = 76;
const BASE_OUTER_RADIUS = 148;
const BASE_CENTER_RADIUS = 52;

function playHoverTick() {
  try {
    const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
    if (!AudioCtx) return;
    const ctx = new AudioCtx();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(1400, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(300, ctx.currentTime + 0.02);
    gain.gain.setValueAtTime(0.04, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.02);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 0.025);
  } catch {}
}

export function filterActions(
  actions: ActionManifest[],
  page: WheelPage,
  extensions: string[],
  contextFilterEnabled = true
): ActionManifest[] {
  const categoryActions = actions
    .filter((a) => {
      if (page === "convert" && a.category !== "convert") return false;
      if (page === "tools" && a.category !== "tools") return false;
      return a.enabled;
    })
    .sort((a, b) => a.order - b.order);

  if (categoryActions.length === 0) {
    return [];
  }

  // Context filter: if enabled and extensions exist, show actions that accept them
  if (contextFilterEnabled && extensions.length > 0) {
    const cleanExts = extensions.map((e) => e.toLowerCase().trim().replace(/^\./, ""));
    const matched = categoryActions.filter((a) => {
      if (a.accepts.extensions.length === 0) return true;
      return a.accepts.extensions.some((ext) => cleanExts.includes(ext.toLowerCase()));
    });
    if (matched.length > 0) {
      return matched.slice(0, 8);
    }
  }

  // Fallback: show all actions in this category so the wheel is never empty
  return categoryActions.slice(0, 8);
}

function wedgeGeometry(
  index: number,
  total: number,
  outerR: number,
  innerR: number,
  cx: number,
  cy: number
) {
  const angleStep = (2 * Math.PI) / total;
  // Start at top (-90°)
  const startAngle = index * angleStep - Math.PI / 2;
  const endAngle = startAngle + angleStep;
  const midAngle = (startAngle + endAngle) / 2;
  const gap = 0.04; // radians gap between wedges

  const sa = startAngle + gap / 2;
  const ea = endAngle - gap / 2;

  const x1 = cx + outerR * Math.cos(sa);
  const y1 = cy + outerR * Math.sin(sa);
  const x2 = cx + outerR * Math.cos(ea);
  const y2 = cy + outerR * Math.sin(ea);
  const x3 = cx + innerR * Math.cos(ea);
  const y3 = cy + innerR * Math.sin(ea);
  const x4 = cx + innerR * Math.cos(sa);
  const y4 = cy + innerR * Math.sin(sa);

  const largeArc = angleStep > Math.PI ? 1 : 0;

  const path =
    `M ${x1} ${y1} ` +
    `A ${outerR} ${outerR} 0 ${largeArc} 1 ${x2} ${y2} ` +
    `L ${x3} ${y3} ` +
    `A ${innerR} ${innerR} 0 ${largeArc} 0 ${x4} ${y4} ` +
    `Z`;

  // Label position: center of wedge arc
  const labelR = (outerR + innerR) / 2;
  const labelX = cx + labelR * Math.cos(midAngle);
  const labelY = cy + labelR * Math.sin(midAngle);

  return { path, labelX, labelY, midAngle };
}

/** Determine which wedge the cursor is over based on its position relative to wheel center */
export function hitTestWedge(
  cursorX: number,
  cursorY: number,
  totalWedges: number,
  wheelCenterX: number,
  wheelCenterY: number,
  outerRadius = BASE_OUTER_RADIUS,
  innerRadius = BASE_INNER_RADIUS
): number | null {
  const dx = cursorX - wheelCenterX;
  const dy = cursorY - wheelCenterY;
  const dist = Math.sqrt(dx * dx + dy * dy);

  if (dist < innerRadius || dist > outerRadius) return null;

  const angle = Math.atan2(dy, dx) + Math.PI / 2; // offset so 0 is top
  const normalized = ((angle % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const index = Math.floor((normalized / (2 * Math.PI)) * totalWedges);
  return index;
}

function RadialWheelInner({
  files,
  extensions,
  currentPage,
  hoveredWedge,
  onTogglePage,
  onWedgeHover,
  onWedgeDrop,
  size = DEFAULT_WHEEL_SIZE,
  contextFilterEnabled = true,
  soundEnabled = false,
}: RadialWheelProps) {
  const actions = useWheelStore((s) => s.actions);
  const visibleActions = useMemo(
    () => filterActions(actions, currentPage, extensions, contextFilterEnabled),
    [actions, currentPage, extensions, contextFilterEnabled]
  );

  const wheelSize = size || DEFAULT_WHEEL_SIZE;
  const scale = wheelSize / DEFAULT_WHEEL_SIZE;
  const outerR = BASE_OUTER_RADIUS * scale;
  const innerR = BASE_INNER_RADIUS * scale;
  const centerR = BASE_CENTER_RADIUS * scale;
  const cx = wheelSize / 2;
  const cy = wheelSize / 2;

  const wedgeGeometries = useMemo(() => {
    return visibleActions.map((action, i) => ({
      action,
      ...wedgeGeometry(i, visibleActions.length, outerR, innerR, cx, cy),
    }));
  }, [visibleActions, outerR, innerR, cx, cy]);

  const hoveredAction = useMemo(
    () => visibleActions.find((a) => a.id === hoveredWedge),
    [visibleActions, hoveredWedge]
  );

  return (
    <div
      className="relative select-none"
      style={{ width: wheelSize, height: wheelSize }}
      onMouseMove={(e) => {
        const rect = e.currentTarget.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const idx = hitTestWedge(mx, my, visibleActions.length, cx, cy, outerR, innerR);
        const nextId = idx !== null ? visibleActions[idx]?.id ?? null : null;
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
      {/* Background glow */}
      <div
        className="absolute inset-0 rounded-full pointer-events-none"
        style={{
          background: "radial-gradient(circle, hsla(22, 95%, 55%, 0.12) 0%, transparent 70%)",
        }}
      />

      <svg
        width={wheelSize}
        height={wheelSize}
        className="absolute inset-0"
        style={{
          filter: "drop-shadow(0 8px 24px rgba(0,0,0,0.5))",
          willChange: "transform",
        }}
      >
        <defs>
          {/* Gradient for normal wedge fill */}
          <linearGradient id="wedge-grad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="hsla(240, 15%, 18%, 0.92)" />
            <stop offset="100%" stopColor="hsla(240, 15%, 12%, 0.88)" />
          </linearGradient>
          {/* Gradient for hovered wedge */}
          <linearGradient id="wedge-active" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="hsla(22, 95%, 60%, 0.95)" />
            <stop offset="100%" stopColor="hsla(355, 85%, 55%, 0.92)" />
          </linearGradient>
        </defs>

        {/* Outer ring */}
        <circle
          cx={cx}
          cy={cy}
          r={outerR + 2}
          fill="none"
          stroke="hsla(0,0%,100%,0.08)"
          strokeWidth="1"
        />
        {/* Inner ring */}
        <circle
          cx={cx}
          cy={cy}
          r={innerR - 2}
          fill="none"
          stroke="hsla(0,0%,100%,0.06)"
          strokeWidth="1"
        />

        {/* Wedges */}
        {wedgeGeometries.map(({ action, path, labelX, labelY }) => {
          const isHovered = hoveredWedge === action.id;

          return (
            <g key={action.id} style={{ cursor: "pointer" }}>
              <path
                d={path}
                fill={isHovered ? "url(#wedge-active)" : "url(#wedge-grad)"}
                stroke={isHovered ? "hsla(22, 95%, 65%, 0.75)" : "hsla(0,0%,100%,0.1)"}
                strokeWidth={isHovered ? 1.5 : 1}
                style={{
                  transformOrigin: `${cx}px ${cy}px`,
                  transform: isHovered ? "scale(1.04)" : "scale(1)",
                  opacity: isHovered ? 1 : 0.88,
                  transition: "transform 0.08s cubic-bezier(0.16, 1, 0.3, 1), fill 0.08s ease, stroke 0.08s ease, opacity 0.08s ease",
                  willChange: "transform",
                }}
              />
              {/* Wedge label */}
              <text
                x={labelX}
                y={labelY}
                textAnchor="middle"
                dominantBaseline="central"
                fill={isHovered ? "#ffffff" : "hsla(0,0%,90%,0.85)"}
                fontSize={action.category === "convert" ? 11 : 10}
                fontWeight={isHovered ? "700" : "600"}
                fontFamily="Inter, system-ui, sans-serif"
                style={{
                  pointerEvents: "none",
                  userSelect: "none",
                  transformOrigin: `${cx}px ${cy}px`,
                  transform: isHovered ? "scale(1.04)" : "scale(1)",
                  transition: "transform 0.08s cubic-bezier(0.16, 1, 0.3, 1), fill 0.08s ease",
                  willChange: "transform",
                }}
              >
                {action.title.toUpperCase()}
              </text>
            </g>
          );
        })}
      </svg>

      {/* Center pill */}
      <div
        className="absolute flex flex-col items-center justify-center rounded-full cursor-pointer pointer-events-auto"
        style={{
          left: cx - centerR,
          top: cy - centerR,
          width: centerR * 2,
          height: centerR * 2,
          background: hoveredAction
            ? "linear-gradient(135deg, hsla(22,95%,50%,0.95), hsla(355,85%,52%,0.95))"
            : "hsla(240,15%,15%,0.92)",
          border: "1.5px solid hsla(0,0%,100%,0.15)",
          backdropFilter: "blur(12px)",
          WebkitBackdropFilter: "blur(12px)",
          boxShadow: hoveredAction
            ? "0 4px 24px hsla(22,95%,55%,0.4), 0 0 0 1px hsla(22,95%,65%,0.3)"
            : "0 4px 16px rgba(0,0,0,0.3)",
          transition: "background 0.1s ease, box-shadow 0.1s ease",
        }}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          onTogglePage();
        }}
        onMouseDown={(e) => {
          // Left, middle, or right click on center pill toggles page
          if (e.button === 0 || e.button === 1 || e.button === 2) {
            e.preventDefault();
            e.stopPropagation();
            onTogglePage();
          }
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
        title="Scroll wheel, Right-Click, or Tab/Space to switch wheels"
      >
        {hoveredAction ? (
          <span
            className="text-white font-bold text-[11px] text-center leading-tight px-1"
            style={{ fontFamily: "Inter, system-ui, sans-serif" }}
          >
            {hoveredAction.title.toUpperCase()}
          </span>
        ) : (
          <>
            <span
              className="text-white/80 text-[10px] font-semibold"
              style={{ fontFamily: "Inter, system-ui, sans-serif" }}
            >
              {files.length > 0 ? `${files.length} file${files.length > 1 ? "s" : ""}` : "DROP"}
            </span>
            {/* Category label */}
            <span
              className="text-[9px] uppercase tracking-wider font-bold mt-0.5"
              style={{
                color: currentPage === "convert" ? "hsla(22,95%,65%,1)" : "hsla(210,95%,65%,1)",
                fontFamily: "Inter, system-ui, sans-serif",
              }}
            >
              {currentPage === "convert" ? "CONVERT" : "TOOLS"}
            </span>
            {/* Page dots */}
            <div className="flex gap-1.5 mt-0.5">
              <div
                className="rounded-full"
                style={{
                  width: 5,
                  height: 5,
                  background: currentPage === "convert" ? "hsla(22,95%,60%,1)" : "hsla(0,0%,100%,0.25)",
                  transition: "background 0.2s",
                }}
              />
              <div
                className="rounded-full"
                style={{
                  width: 5,
                  height: 5,
                  background: currentPage === "tools" ? "hsla(22,95%,60%,1)" : "hsla(0,0%,100%,0.25)",
                  transition: "background 0.2s",
                }}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export const RadialWheel = React.memo(RadialWheelInner);

