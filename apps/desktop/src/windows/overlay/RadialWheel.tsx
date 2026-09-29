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
}

const WHEEL_SIZE = 320;
const INNER_RADIUS = 76;
const OUTER_RADIUS = 148;
const CENTER_RADIUS = 52;

export function filterActions(
  actions: ActionManifest[],
  page: WheelPage,
  extensions: string[]
): ActionManifest[] {
  return actions
    .filter((a) => {
      if (page === "convert" && a.category !== "convert") return false;
      if (page === "tools" && a.category !== "tools") return false;
      if (!a.enabled) return false;
      // Context filter: only show if the action accepts at least one dragged ext
      if (extensions.length > 0 && a.accepts.extensions.length > 0) {
        return a.accepts.extensions.some((e) => extensions.includes(e));
      }
      return true;
    })
    .sort((a, b) => a.order - b.order)
    .slice(0, 8);
}

function wedgeGeometry(index: number, total: number, outerR: number, innerR: number) {
  const angleStep = (2 * Math.PI) / total;
  // Start at top (-90°)
  const startAngle = index * angleStep - Math.PI / 2;
  const endAngle = startAngle + angleStep;
  const midAngle = (startAngle + endAngle) / 2;
  const gap = 0.04; // radians gap between wedges

  const sa = startAngle + gap / 2;
  const ea = endAngle - gap / 2;
  const cx = WHEEL_SIZE / 2;
  const cy = WHEEL_SIZE / 2;

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
  wheelCenterY: number
): number | null {
  const dx = cursorX - wheelCenterX;
  const dy = cursorY - wheelCenterY;
  const dist = Math.sqrt(dx * dx + dy * dy);

  if (dist < INNER_RADIUS || dist > OUTER_RADIUS) return null;

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
}: RadialWheelProps) {
  const actions = useWheelStore((s) => s.actions);
  const visibleActions = useMemo(
    () => filterActions(actions, currentPage, extensions),
    [actions, currentPage, extensions]
  );

  const cx = WHEEL_SIZE / 2;
  const cy = WHEEL_SIZE / 2;

  const wedgeGeometries = useMemo(() => {
    return visibleActions.map((action, i) => ({
      action,
      ...wedgeGeometry(i, visibleActions.length, OUTER_RADIUS, INNER_RADIUS),
    }));
  }, [visibleActions]);

  const hoveredAction = useMemo(
    () => visibleActions.find((a) => a.id === hoveredWedge),
    [visibleActions, hoveredWedge]
  );

  return (
    <div
      className="relative select-none"
      style={{ width: WHEEL_SIZE, height: WHEEL_SIZE }}
      onMouseMove={(e) => {
        const rect = e.currentTarget.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const idx = hitTestWedge(mx, my, visibleActions.length, cx, cy);
        const nextId = idx !== null ? visibleActions[idx]?.id ?? null : null;
        if (nextId !== hoveredWedge) {
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
        width={WHEEL_SIZE}
        height={WHEEL_SIZE}
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
          r={OUTER_RADIUS + 2}
          fill="none"
          stroke="hsla(0,0%,100%,0.08)"
          strokeWidth="1"
        />
        {/* Inner ring */}
        <circle
          cx={cx}
          cy={cy}
          r={INNER_RADIUS - 2}
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
          left: cx - CENTER_RADIUS,
          top: cy - CENTER_RADIUS,
          width: CENTER_RADIUS * 2,
          height: CENTER_RADIUS * 2,
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
        onClick={onTogglePage}
        title="Click to switch page (or Tab/Space)"
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
              className="text-white/60 text-[10px] font-semibold"
              style={{ fontFamily: "Inter, system-ui, sans-serif" }}
            >
              {files.length > 0 ? `${files.length} file${files.length > 1 ? "s" : ""}` : "DROP"}
            </span>
            {/* Page dots */}
            <div className="flex gap-1 mt-1">
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

