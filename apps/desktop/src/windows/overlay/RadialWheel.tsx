import React, { useMemo } from "react";
import { ActionManifest, WheelPage } from "../../store/wheelStore";
import { playHoverSound } from "../../utils/sound";

export interface PetalData {
  id: string;
  action: string;
  title: string;
  d: string;
  labelX: number;
  labelY: number;
  subtitle: string;
  disabled?: boolean;
  icon?: {
    x: number;
    y: number;
    width: number;
    height: number;
    type: "trim";
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
    id: "convert.jpg",
    action: "jpg",
    title: "JPG",
    d: "M 51.7498 19.2707 L 92.5434 36.1680 Q 111.1720 43.8842 116.4816 26.7781 A 119.52000000000001 119.52000000000001 0 0 0 116.4816 -26.7781 Q 111.1720 -43.8842 92.5434 -36.1680 L 51.7498 -19.2707 Q 46.0275 -16.9004 47.4139 -10.8521 A 48.64000000000001 48.64000000000001 0 0 1 47.4139 10.8521 Q 46.0275 16.9004 51.7498 19.2707 Z",
    labelX: 84.5312,
    labelY: 0,
    subtitle: "Convert to JPG",
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
    id: "convert.ico",
    action: "ico",
    title: "ICO",
    d: "M -22.9662 -50.2191 L -39.8635 -91.0127 Q -47.5797 -109.6413 -63.4300 -101.2999 A 119.52000000000001 119.52000000000001 0 0 0 -101.2999 -63.4300 Q -109.6413 -47.5797 -91.0127 -39.8635 L -50.2191 -22.9662 Q -44.4968 -20.5960 -41.2003 -25.8531 A 48.64000000000001 48.64000000000001 0 0 1 -25.8531 -41.2003 Q -20.5960 -44.4968 -22.9662 -50.2191 Z",
    labelX: -59.7726,
    labelY: -59.7726,
    subtitle: "Convert to ICO",
  },
];

export const TOOLS_PETALS: PetalData[] = [
  {
    id: "tool.trim",
    action: "trim",
    title: "TRIM",
    // 360-degree smooth circular ring with inner radius 48.64 and outer radius 119.52
    d: "M 0 -119.52 A 119.52 119.52 0 1 0 0 119.52 A 119.52 119.52 0 1 0 0 -119.52 M 0 -48.64 A 48.64 48.64 0 1 1 0 48.64 A 48.64 48.64 0 1 1 0 -48.64 Z",
    icon: { x: -10, y: -98, width: 20, height: 20, type: "trim" },
    labelX: 0,
    labelY: -72,
    subtitle: "Trim Blank Pixels",
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
  const count = isCount ? pageOrCount : pageOrCount === "convert" ? 8 : 1;
  const scale =
    isCount && _innerRadius !== undefined
      ? scaleOrOuter / 148
      : typeof scaleOrOuter === "number"
      ? scaleOrOuter
      : 1;

  const dx = (cursorX - wheelCenterX) / (scale || 1);
  const dy = (cursorY - wheelCenterY) / (scale || 1);
  const dist = Math.sqrt(dx * dx + dy * dy);

  // Inner petal boundary is 44, outer boundary is 125
  if (dist < 44 || dist > 125) return null;

  if (count === 1) {
    return 0;
  }

  const angle = Math.atan2(dy, dx);

  if (count === 8) {
    let normalized = (angle + Math.PI / 2 + Math.PI / 8) % (2 * Math.PI);
    if (normalized < 0) normalized += 2 * Math.PI;
    const index = Math.floor(normalized / (Math.PI / 4));
    return index >= 0 && index < 8 ? index : null;
  } else {
    let normalized = (angle + Math.PI / 2 + Math.PI / count) % (2 * Math.PI);
    if (normalized < 0) normalized += 2 * Math.PI;
    const index = Math.floor(normalized / ((2 * Math.PI) / count));
    return index >= 0 && index < count ? index : null;
  }
}

export const RASTER_IMAGE_EXTS = new Set([
  "png", "jpg", "jpeg", "webp", "avif", "tiff", "tif", "bmp", "gif", "ico", "heic", "svg"
]);
export const DOCUMENT_EXTS = new Set(["pdf"]);
export const MEDIA_EXTS = new Set(["mp4", "mov", "mkv", "webm", "avi"]);

export function isPetalEnabledForExtensions(
  actionId: string,
  page: WheelPage,
  extensions: string[]
): boolean {
  if (extensions.length === 0) return true;
  const exts = extensions.map((e) => e.toLowerCase().trim()).filter(Boolean);
  if (exts.length === 0) return true;

  const isImage = exts.some((e) => RASTER_IMAGE_EXTS.has(e));
  const isPdf = exts.some((e) => DOCUMENT_EXTS.has(e));
  const isMedia = exts.some((e) => MEDIA_EXTS.has(e));

  if (page === "convert") {
    const targetFormat = actionId.replace("convert.", "").toLowerCase();

    if (isImage) {
      // Keep option enabled even if it is the same extension
      return true;
    }

    if (isPdf) {
      return ["png", "jpg", "webp", "tiff", "bmp", "pdf"].includes(targetFormat);
    }

    if (isMedia) {
      return ["gif", "mp4", "webm"].includes(targetFormat);
    }

    return false;
  } else {
    // Tools page (TRIM operates on transparent/raster images)
    return isImage;
  }
}


export function filterActions(
  actions: ActionManifest[],
  page: WheelPage,
  extensions: string[] = [],
  contextFilterEnabled = true
): ActionManifest[] {
  const petals = page === "convert" ? CONVERT_PETALS : TOOLS_PETALS;
  return petals.map((p, idx) => {
    const existing = actions.find((a) => a.id === p.id);
    const enabled = contextFilterEnabled
      ? isPetalEnabledForExtensions(p.id, page, extensions)
      : true;

    if (existing) {
      return { ...existing, enabled: existing.enabled && enabled };
    }
    return {
      id: p.id,
      title: p.title,
      icon: p.id,
      category: page === "convert" ? "convert" : "tools",
      accepts: { extensions: [], multi: true },
      kind: page === "convert" ? "instant" : "window",
      enabled,
      order: idx,
    } as ActionManifest;
  });
}


function ToolIcon({
  type,
  stroke = "#222428",
}: {
  type: string;
  stroke?: string;
}) {
  switch (type) {
    case "trim":
      return (
        <g fill="none" stroke={stroke} strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          {/* 4 outer framing corners */}
          <path d="M4 8V4h4" />
          <path d="M20 8V4h-4" />
          <path d="M4 16v4h4" />
          <path d="M20 16v4h-4" />
          {/* Inner tight content box */}
          <rect x="7" y="7" width="10" height="10" rx="1.5" fill={stroke} fillOpacity="0.25" />
        </g>
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
  extensions = [],
  currentPage,
  hoveredWedge,
  onTogglePage,
  onWedgeHover,
  onWedgeDrop,
  size = 272,
  contextFilterEnabled = true,
  soundEnabled = false,
}: RadialWheelProps) {
  const activePetals = useMemo(() => {
    const basePetals = currentPage === "convert" ? CONVERT_PETALS : TOOLS_PETALS;
    return basePetals.map((p) => {
      const isEnabled = contextFilterEnabled
        ? isPetalEnabledForExtensions(p.id, currentPage, extensions)
        : true;
      return {
        ...p,
        disabled: !isEnabled,
      };
    });
  }, [currentPage, contextFilterEnabled, extensions]);

  const hoveredPetal = useMemo(
    () => activePetals.find((p) => p.id === hoveredWedge && !p.disabled) ?? null,
    [activePetals, hoveredWedge]
  );

  const wheelSize = size || 272;
  const scale = wheelSize / 272;

  return (
    <div
      className="relative select-none pointer-events-auto"
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
        const candidate = idx !== null ? activePetals[idx] : null;
        const nextId = candidate && !candidate.disabled ? candidate.id : null;
        if (nextId !== hoveredWedge) {
          if (soundEnabled && nextId !== null) {
            playHoverSound();
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
        if (hoveredPetal && !hoveredPetal.disabled && files.length > 0) {
          onWedgeDrop(hoveredPetal.id, files);
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
            const isDisabled = petal.disabled === true;
            const isHighlighted = hoveredWedge === petal.id && !isDisabled;
            return (
              <g
                key={petal.id}
                className={`preview-target ${isHighlighted ? "is-highlighted" : ""} ${isDisabled ? "is-disabled opacity-30" : ""}`}
                data-action={petal.action}
                role="button"
                aria-label={`Preview ${petal.title}`}
                aria-pressed={isHighlighted}
                aria-disabled={isDisabled}
                style={{ cursor: isDisabled ? "not-allowed" : "pointer" }}
                onMouseEnter={() => {
                  if (isDisabled) return;
                  if (soundEnabled && hoveredWedge !== petal.id) {
                    playHoverSound();
                  }
                  onWedgeHover(petal.id);
                }}
                onClick={() => {
                  if (!isDisabled && files.length > 0) {
                    onWedgeDrop(petal.id, files);
                  }
                }}
              >
                <path
                  d={petal.d}
                  className="preview-petal"
                  fill={
                    isDisabled
                      ? "rgba(220, 225, 230, 0.25)"
                      : isHighlighted
                      ? "url(#petal-active-gradient)"
                      : "url(#conversion-fan-glass)"
                  }
                  stroke={
                    isDisabled
                      ? "rgba(255, 255, 255, 0.2)"
                      : isHighlighted
                      ? "#ff7538"
                      : "rgba(255, 255, 255, 0.85)"
                  }
                  strokeWidth={isHighlighted ? 1.5 : 1.2}
                  style={{
                    filter: isDisabled
                      ? "none"
                      : isHighlighted
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
                    transform={`translate(${petal.icon.x}, ${petal.icon.y}) scale(${petal.icon.width / 24})`}
                    className="wheel-tool-icon preview-tool-icon"
                    style={{
                      pointerEvents: "none",
                      userSelect: "none",
                    }}
                  >
                    <ToolIcon
                      type={petal.icon.type}
                      stroke={isHighlighted ? "#ffffff" : "#222428"}
                    />
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

                {petal.id === "tool.trim" && (
                  <text
                    x={0}
                    y={76}
                    textAnchor="middle"
                    dominantBaseline="central"
                    fill={isHighlighted ? "rgba(255, 255, 255, 0.9)" : "#4b5563"}
                    fontSize="9.5px"
                    fontWeight="700"
                    fontFamily="system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
                    letterSpacing="0.06em"
                    style={{
                      pointerEvents: "none",
                      userSelect: "none",
                      transformOrigin: "0px 0px",
                      transform: isHighlighted ? "scale(1.025)" : "scale(1)",
                      transition: "fill 0.12s ease, transform 0.12s ease",
                    }}
                  >
                    TRIM BLANK PIXELS
                  </text>
                )}
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
                <image
                  href="/logo.svg"
                  x="-16"
                  y="-26"
                  width="32"
                  height="32"
                  preserveAspectRatio="xMidYMid meet"
                  style={{ pointerEvents: "none", userSelect: "none" }}
                />
                <text
                  x="0"
                  y="10"
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill="#2d3135"
                  fontSize="8.5px"
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
                  cy="20"
                  r="1.8"
                  fill={currentPage === "convert" ? "#ff5419" : "#d1d5db"}
                />
                <circle
                  cx="4"
                  cy="20"
                  r="1.8"
                  fill={currentPage === "tools" ? "#ff5419" : "#d1d5db"}
                />
              </>
            )}
          </g>
        </svg>
    </div>
  );
}

export const RadialWheel = React.memo(RadialWheelInner);
