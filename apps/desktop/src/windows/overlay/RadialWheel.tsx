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
  disabled?: boolean;
  icon?: {
    x?: number;
    y?: number;
    width?: number;
    height?: number;
    type: string;
  };
}

export interface PetalDef {
  id: string;
  action: string;
  title: string;
  subtitle: string;
  category?: "convert" | "tools";
  icon?: {
    type: string;
    x?: number;
    y?: number;
    width?: number;
    height?: number;
  };
  /** Extensions this action natively supports. Omit to accept all in category. */
  supportedExtensions?: Set<string>;
  /** Optional custom predicate for fine-grained contextual matching */
  isApplicable?: (extensions: string[]) => boolean;
}

export interface PetalGeometry {
  d: string;
  labelX: number;
  labelY: number;
}

// -------------------------------------------------------------------------
// Supported Extensions Catalogs (Modular & easy to add/remove)
// -------------------------------------------------------------------------

// Image formats supporting an alpha channel / transparency
export const TRANSPARENT_IMAGE_EXTS = new Set([
  "png", "webp", "avif", "tiff", "tif", "gif", "ico", "svg", "heic"
]);

// Opaque image formats with no alpha channel
export const OPAQUE_IMAGE_EXTS = new Set([
  "jpg", "jpeg", "bmp"
]);

// Combined image formats (no new extensions added)
export const RASTER_IMAGE_EXTS = new Set([
  ...TRANSPARENT_IMAGE_EXTS,
  ...OPAQUE_IMAGE_EXTS,
]);

export const DOCUMENT_EXTS = new Set(["pdf"]);

// 5 most common video extensions
export const VIDEO_EXTS = new Set([
  "mp4", "mov", "mkv", "webm", "avi"
]);

// 4-5 most common audio extensions
export const AUDIO_EXTS = new Set([
  "mp3", "wav", "m4a", "flac", "aac"
]);

// Combined media extensions for quick verification
export const MEDIA_EXTS = new Set([
  ...VIDEO_EXTS,
  ...AUDIO_EXTS,
]);

/**
 * Normalizes common extension aliases (e.g. jpeg -> jpg, tif -> tiff).
 */
export function normalizeExt(ext: string): string {
  const clean = ext.toLowerCase().trim();
  if (clean === "jpeg") return "jpg";
  if (clean === "tif") return "tiff";
  return clean;
}

/**
 * Fast O(1) predicate checking if an action is applicable for dragged file extensions.
 */
export function isActionApplicable(def: PetalDef, extensions: string[]): boolean {
  if (!extensions || extensions.length === 0) return true;
  const exts = extensions.map((e) => e.toLowerCase().trim()).filter(Boolean);
  if (exts.length === 0) return true;

  if (def.isApplicable) {
    return def.isApplicable(exts);
  }

  if (def.supportedExtensions) {
    return exts.some((e) => def.supportedExtensions!.has(e));
  }

  return true;
}

// -------------------------------------------------------------------------
// Target Catalogs for Dynamic Context Resolution
// -------------------------------------------------------------------------

export const IMAGE_CONVERT_CATALOG: PetalDef[] = [
  { id: "convert.png", action: "png", title: "PNG", subtitle: "Convert to PNG", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.webp", action: "webp", title: "WEBP", subtitle: "Convert to WEBP", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.jpg", action: "jpg", title: "JPG", subtitle: "Convert to JPG", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.tiff", action: "tiff", title: "TIFF", subtitle: "Convert to TIFF", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.avif", action: "avif", title: "AVIF", subtitle: "Convert to AVIF", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.bmp", action: "bmp", title: "BMP", subtitle: "Convert to BMP", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.pdf", action: "pdf", title: "PDF", subtitle: "Convert to PDF", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.ico", action: "ico", title: "ICO", subtitle: "Convert to ICO", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.gif", action: "gif", title: "GIF", subtitle: "Convert to GIF", supportedExtensions: RASTER_IMAGE_EXTS },
  { id: "convert.heic", action: "heic", title: "HEIC", subtitle: "Convert to HEIC", supportedExtensions: RASTER_IMAGE_EXTS },
];

export const VIDEO_CONVERT_CATALOG: PetalDef[] = [
  { id: "convert.mp4", action: "mp4", title: "MP4", subtitle: "Convert to MP4", supportedExtensions: VIDEO_EXTS },
  { id: "convert.webm", action: "webm", title: "WEBM", subtitle: "Convert to WebM", supportedExtensions: VIDEO_EXTS },
  { id: "convert.gif", action: "gif", title: "GIF", subtitle: "Create Animated GIF", supportedExtensions: VIDEO_EXTS },
  { id: "convert.mp3", action: "mp3", title: "MP3", subtitle: "Extract MP3 Audio", supportedExtensions: MEDIA_EXTS },
  { id: "convert.wav", action: "wav", title: "WAV", subtitle: "Extract WAV Audio", supportedExtensions: MEDIA_EXTS },
  { id: "convert.mov", action: "mov", title: "MOV", subtitle: "Convert to QuickTime MOV", supportedExtensions: VIDEO_EXTS },
  { id: "convert.mkv", action: "mkv", title: "MKV", subtitle: "Convert to Matroska MKV", supportedExtensions: VIDEO_EXTS },
  { id: "convert.m4a", action: "m4a", title: "M4A", subtitle: "Extract M4A Audio", supportedExtensions: MEDIA_EXTS },
  { id: "convert.avi", action: "avi", title: "AVI", subtitle: "Convert to AVI", supportedExtensions: VIDEO_EXTS },
  { id: "convert.flac", action: "flac", title: "FLAC", subtitle: "Extract Lossless FLAC", supportedExtensions: MEDIA_EXTS },
];

export const AUDIO_CONVERT_CATALOG: PetalDef[] = [
  { id: "convert.mp3", action: "mp3", title: "MP3", subtitle: "Convert to MP3", supportedExtensions: AUDIO_EXTS },
  { id: "convert.wav", action: "wav", title: "WAV", subtitle: "Convert to WAV", supportedExtensions: AUDIO_EXTS },
  { id: "convert.m4a", action: "m4a", title: "M4A", subtitle: "Convert to M4A", supportedExtensions: AUDIO_EXTS },
  { id: "convert.flac", action: "flac", title: "FLAC", subtitle: "Convert to Lossless FLAC", supportedExtensions: AUDIO_EXTS },
  { id: "convert.aac", action: "aac", title: "AAC", subtitle: "Convert to AAC", supportedExtensions: AUDIO_EXTS },
  { id: "convert.ogg", action: "ogg", title: "OGG", subtitle: "Convert to OGG", supportedExtensions: AUDIO_EXTS },
  { id: "convert.opus", action: "opus", title: "OPUS", subtitle: "Convert to OPUS", supportedExtensions: AUDIO_EXTS },
  { id: "convert.wma", action: "wma", title: "WMA", subtitle: "Convert to WMA", supportedExtensions: AUDIO_EXTS },
];

export const DOCUMENT_CONVERT_CATALOG: PetalDef[] = [
  { id: "convert.png", action: "png", title: "PNG", subtitle: "Extract to PNG", supportedExtensions: DOCUMENT_EXTS },
  { id: "convert.jpg", action: "jpg", title: "JPG", subtitle: "Extract to JPG", supportedExtensions: DOCUMENT_EXTS },
  { id: "convert.webp", action: "webp", title: "WEBP", subtitle: "Extract to WEBP", supportedExtensions: DOCUMENT_EXTS },
  { id: "convert.tiff", action: "tiff", title: "TIFF", subtitle: "Extract to TIFF", supportedExtensions: DOCUMENT_EXTS },
  { id: "convert.bmp", action: "bmp", title: "BMP", subtitle: "Extract to BMP", supportedExtensions: DOCUMENT_EXTS },
  { id: "convert.pdf", action: "pdf", title: "PDF", subtitle: "Combine/Save PDF", supportedExtensions: RASTER_IMAGE_EXTS },
];

// Only TRIM tool kept — strictly accepts transparent background compatible image files
export const TOOLS_CATALOG: PetalDef[] = [
  {
    id: "tool.trim",
    action: "trim",
    title: "TRIM",
    subtitle: "Trim Blank Pixels",
    icon: { type: "trim" },
    supportedExtensions: TRANSPARENT_IMAGE_EXTS,
  },
];

// -------------------------------------------------------------------------
// Accurate Mathematical Petal Geometry Generation (Based on Generator Engine)
// -------------------------------------------------------------------------

/**
 * Computes an annular sector path with smooth tangent continuous fillets at all 4 corners.
 * Matches the reference SVG generator algorithm adapted to (cx, cy) = (0, 0).
 */
export function computeFilletedPetalPath(
  cx: number,
  cy: number,
  rIn: number,
  rOut: number,
  a1: number,
  a2: number,
  rf: number
): string {
  while (a2 < a1) a2 += Math.PI * 2;
  const angleSpan = a2 - a1;
  const radialThickness = rOut - rIn;

  // Outer fillet radius bound
  const rfOutMax = Math.min(radialThickness * 0.48, rOut * angleSpan * 0.45);
  const rfOut = Math.max(0.5, Math.min(rf, rfOutMax));

  // Inner fillet radius proportionally scaled to prevent pinch
  const rfInMax = Math.min(radialThickness * 0.48, rIn * angleSpan * 0.45);
  const rfIn = Math.max(0.5, Math.min(rf * (rIn / rOut) * 1.15, rfInMax));

  // 1. Outer Fillet Math:
  const sinDeltaOut = Math.min(0.999, rfOut / (rOut - rfOut));
  const deltaOut = Math.asin(sinDeltaOut);
  const dRayOut = Math.sqrt(Math.max(0, (rOut - rfOut) * (rOut - rfOut) - rfOut * rfOut));

  const phiOut1 = a1 + deltaOut;
  const phiOut2 = a2 - deltaOut;

  const pRay1Out = { x: cx + dRayOut * Math.cos(a1), y: cy + dRayOut * Math.sin(a1) };
  const pArcOut1 = { x: cx + rOut * Math.cos(phiOut1), y: cy + rOut * Math.sin(phiOut1) };
  const pArcOut2 = { x: cx + rOut * Math.cos(phiOut2), y: cy + rOut * Math.sin(phiOut2) };
  const pRay2Out = { x: cx + dRayOut * Math.cos(a2), y: cy + dRayOut * Math.sin(a2) };

  // 2. Inner Fillet Math:
  const sinDeltaIn = Math.min(0.999, rfIn / (rIn + rfIn));
  const deltaIn = Math.asin(sinDeltaIn);
  const dRayIn = Math.sqrt(Math.max(0, (rIn + rfIn) * (rIn + rfIn) - rfIn * rfIn));

  const phiIn1 = a1 + deltaIn;
  const phiIn2 = a2 - deltaIn;

  const pRay2In = { x: cx + dRayIn * Math.cos(a2), y: cy + dRayIn * Math.sin(a2) };
  const pArcIn2 = { x: cx + rIn * Math.cos(phiIn2), y: cy + rIn * Math.sin(phiIn2) };
  const pArcIn1 = { x: cx + rIn * Math.cos(phiIn1), y: cy + rIn * Math.sin(phiIn1) };
  const pRay1In = { x: cx + dRayIn * Math.cos(a1), y: cy + dRayIn * Math.sin(a1) };

  const f = (n: number) => Number(n.toFixed(2));

  const outerArcLarge = phiOut2 - phiOut1 > Math.PI ? 1 : 0;
  const innerArcLarge = phiIn2 - phiIn1 > Math.PI ? 1 : 0;

  return [
    `M ${f(pRay1In.x)} ${f(pRay1In.y)}`,
    `L ${f(pRay1Out.x)} ${f(pRay1Out.y)}`,
    `A ${f(rfOut)} ${f(rfOut)} 0 0 1 ${f(pArcOut1.x)} ${f(pArcOut1.y)}`,
    `A ${f(rOut)} ${f(rOut)} 0 ${outerArcLarge} 1 ${f(pArcOut2.x)} ${f(pArcOut2.y)}`,
    `A ${f(rfOut)} ${f(rfOut)} 0 0 1 ${f(pRay2Out.x)} ${f(pRay2Out.y)}`,
    `L ${f(pRay2In.x)} ${f(pRay2In.y)}`,
    `A ${f(rfIn)} ${f(rfIn)} 0 0 1 ${f(pArcIn2.x)} ${f(pArcIn2.y)}`,
    `A ${f(rIn)} ${f(rIn)} 0 ${innerArcLarge} 0 ${f(pArcIn1.x)} ${f(pArcIn1.y)}`,
    `A ${f(rfIn)} ${f(rfIn)} 0 0 1 ${f(pRay1In.x)} ${f(pRay1In.y)}`,
    "Z",
  ].join(" ");
}

/**
 * Calculates exact petal geometry for radial sectors.
 * Preserves the exact radii and spacing of Lime's wheel:
 * cx = 0, cy = 0 within viewBox="-136 -136 272 272"
 * rIn = 48.64, rOut = 119.52
 */
export function calculatePetalGeometry(
  index: number,
  count: number,
  cx = 0,
  cy = 0,
  rIn = 48.64,
  rOut = 119.52,
  petalGap = 4.2,
  cornerRoundness = 7.5
): PetalGeometry {
  const midRadius = (rIn + rOut) / 2;

  // Single petal: full 360-degree donut ring
  if (count <= 1) {
    const d = [
      `M ${cx} ${cy - rOut}`,
      `A ${rOut} ${rOut} 0 1 0 ${cx} ${cy + rOut}`,
      `A ${rOut} ${rOut} 0 1 0 ${cx} ${cy - rOut}`,
      `M ${cx} ${cy - rIn}`,
      `A ${rIn} ${rIn} 0 1 1 ${cx} ${cy + rIn}`,
      `A ${rIn} ${rIn} 0 1 1 ${cx} ${cy - rIn}`,
      "Z",
    ].join(" ");
    return {
      d,
      labelX: cx,
      labelY: cy - midRadius,
    };
  }

  const angularGap = petalGap / midRadius;
  const angleStep = (2 * Math.PI) / count;
  const halfSpan = (angleStep - angularGap) / 2;

  // Center angle: Index 0 starts strictly at 12 o'clock (-PI/2) and goes clockwise
  const centerAngle = -Math.PI / 2 + index * angleStep;
  const a1 = centerAngle - halfSpan;
  const a2 = centerAngle + halfSpan;

  const d = computeFilletedPetalPath(cx, cy, rIn, rOut, a1, a2, cornerRoundness);
  return {
    d,
    labelX: Number((cx + midRadius * Math.cos(centerAngle)).toFixed(2)),
    labelY: Number((cy + midRadius * Math.sin(centerAngle)).toFixed(2)),
  };
}

/**
 * Generate petals dynamically for any page, file extensions, and custom slot count (1-10).
 */
export function getPetalsForPage(
  page: WheelPage,
  extensions: string[] = [],
  customSlotCount = 8,
  contextFilterEnabled = true
): PetalData[] {
  const clampedCount = Math.max(1, Math.min(10, customSlotCount));
  const exts = (extensions || []).map((e) => e.toLowerCase().trim()).filter(Boolean);

  if (page === "tools") {
    // Only tools applicable to dragged extensions
    const defs = contextFilterEnabled && exts.length > 0
      ? TOOLS_CATALOG.filter((def) => isActionApplicable(def, exts))
      : TOOLS_CATALOG;

    if (defs.length === 0) {
      return [];
    }

    const selectedDefs = defs.slice(0, clampedCount);
    const count = selectedDefs.length;

    // Single tool: smooth 360-degree circular ring
    if (count === 1) {
      const geo = calculatePetalGeometry(0, 1);
      return [
        {
          ...selectedDefs[0],
          d: geo.d,
          labelX: geo.labelX,
          labelY: geo.labelY,
          icon: selectedDefs[0].icon || { type: "trim" },
        },
      ];
    }

    return selectedDefs.map((def, i) => {
      const geo = calculatePetalGeometry(i, count);
      return {
        ...def,
        d: geo.d,
        labelX: geo.labelX,
        labelY: geo.labelY,
      };
    });
  }

  // Convert page: filter out non-applicable actions completely
  const fullCatalog = getCatalogForExtensions(exts);
  const normExts = exts.map(normalizeExt);
  const defs = contextFilterEnabled && exts.length > 0
    ? fullCatalog.filter((def) => {
        // Redundant self-conversion: e.g. dragging a single PNG/JPG/MP4 -> don't show conversion to itself
        const normAction = normalizeExt(def.action);
        const isSelf =
          normExts.length > 0 &&
          normExts.every((e) => e === normAction || `convert.${e}` === def.id);
        if (isSelf) {
          return false;
        }
        return isActionApplicable(def, exts);
      })
    : fullCatalog;

  const selectedDefs = defs.slice(0, clampedCount);
  const count = selectedDefs.length;

  if (count === 0) {
    return [];
  }

  if (count === 1) {
    const geo = calculatePetalGeometry(0, 1);
    return [
      {
        ...selectedDefs[0],
        d: geo.d,
        labelX: geo.labelX,
        labelY: geo.labelY,
      },
    ];
  }

  return selectedDefs.map((def, i) => {
    const geo = calculatePetalGeometry(i, count);
    return {
      ...def,
      d: geo.d,
      labelX: geo.labelX,
      labelY: geo.labelY,
    };
  });
}

/**
 * Returns whether tools are available for the given dragged extensions.
 */
export function hasToolsForExtensions(
  extensions: string[] = [],
  contextFilterEnabled = true
): boolean {
  const tools = getPetalsForPage("tools", extensions, 10, contextFilterEnabled);
  return tools.length > 0;
}

/**
 * Returns the relevant conversion catalog for the given dragged extensions.
 */
export function getCatalogForExtensions(extensions: string[] = []): PetalDef[] {
  const exts = (extensions || []).map((e) => e.toLowerCase().trim()).filter(Boolean);
  const isAudio = exts.some((e) => AUDIO_EXTS.has(e));
  const isVideo = exts.some((e) => VIDEO_EXTS.has(e));
  const isDoc = exts.some((e) => DOCUMENT_EXTS.has(e));

  if (isAudio && !isVideo) {
    return AUDIO_CONVERT_CATALOG;
  } else if (isVideo) {
    return VIDEO_CONVERT_CATALOG;
  } else if (isDoc) {
    return DOCUMENT_CONVERT_CATALOG;
  } else {
    return IMAGE_CONVERT_CATALOG;
  }
}

// Fallback exports for backward compatibility
export const CONVERT_PETALS: PetalData[] = getPetalsForPage("convert", [], 8);
export const TOOLS_PETALS: PetalData[] = getPetalsForPage("tools", [], 1);

/** Outer diameter in CSS px per wheel size preset. Mirrors `WheelSize::px` in wheel-core. */
export const WHEEL_SIZE_PX: Record<string, number> = {
  small: 280,
  medium: 320,
  large: 400,
};

/** Resolve a wheel size preset name to its pixel diameter, falling back to medium. */
export function wheelDiameter(size: string | undefined | null): number {
  return WHEEL_SIZE_PX[size ?? ""] ?? WHEEL_SIZE_PX.medium;
}

/**
 * Perform exact mathematical hit testing against `count` radial sectors.
 * Inner radius is 48.64, outer radius is 119.52.
 */
export function hitTestWedge(
  cursorX: number,
  cursorY: number,
  pageOrCount: WheelPage | number,
  wheelCenterX = 200,
  wheelCenterY = 200,
  scale = 1
): number | null {
  const count =
    typeof pageOrCount === "number"
      ? pageOrCount
      : pageOrCount === "convert"
      ? 8
      : 1;

  const dx = (cursorX - wheelCenterX) / (scale || 1);
  const dy = (cursorY - wheelCenterY) / (scale || 1);
  const dist = Math.sqrt(dx * dx + dy * dy);

  // Inner petal boundary is 44, outer boundary is 125
  if (dist < 44 || dist > 125) return null;

  if (count <= 0) return null;
  if (count === 1) {
    return 0;
  }

  const angle = Math.atan2(dy, dx);
  const step = (2 * Math.PI) / count;
  const halfStep = step / 2;

  let normalized = (angle + Math.PI / 2 + halfStep) % (2 * Math.PI);
  if (normalized < 0) normalized += 2 * Math.PI;

  const index = Math.floor(normalized / step);
  return index >= 0 && index < count ? index : null;
}

export function isPetalEnabledForExtensions(
  actionId: string,
  page: WheelPage,
  extensions: string[]
): boolean {
  if (!extensions || extensions.length === 0) return true;
  const exts = extensions.map((e) => e.toLowerCase().trim()).filter(Boolean);
  if (exts.length === 0) return true;

  if (page === "tools") {
    const def = TOOLS_CATALOG.find((d) => d.id === actionId);
    return def ? isActionApplicable(def, exts) : false;
  }

  // Convert page: check the specific catalog for dragged extensions
  const catalog = getCatalogForExtensions(exts);
  const def = catalog.find((d) => d.id === actionId);
  if (def) {
    const normExts = exts.map(normalizeExt);
    const normAction = normalizeExt(def.action);
    const isSelf =
      normExts.length > 0 &&
      normExts.every((e) => e === normAction || `convert.${e}` === def.id);
    if (isSelf) {
      return false;
    }
    return isActionApplicable(def, exts);
  }

  return false;
}

export function filterActions(
  actions: ActionManifest[],
  page: WheelPage,
  extensions: string[] = [],
  contextFilterEnabled = true,
  slotCount = 8
): ActionManifest[] {
  const petals = getPetalsForPage(page, extensions, slotCount, contextFilterEnabled);
  return petals.map((p, idx) => {
    const existing = actions.find((a) => a.id === p.id);
    if (existing) {
      return { ...existing, enabled: true };
    }
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

function ToolIcon({
  type,
  stroke = "#222428",
}: {
  type: string;
  stroke?: string;
}) {
  switch (type) {
    case "trim":
    default:
      return (
        <g fill="none" stroke={stroke} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 8V4h4" />
          <path d="M20 8V4h-4" />
          <path d="M4 16v4h4" />
          <path d="M20 16v4h-4" />
          <rect x="7" y="7" width="10" height="10" rx="1.5" fill={stroke} fillOpacity="0.25" />
        </g>
      );
  }
}

export interface RadialWheelProps {
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
  slotCount?: number;
  hasTools?: boolean;
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
  slotCount = 8,
  hasTools: hasToolsProp,
}: RadialWheelProps) {
  const hasTools = hasToolsProp ?? hasToolsForExtensions(extensions, contextFilterEnabled);

  const activePetals = useMemo(() => {
    return getPetalsForPage(currentPage, extensions, slotCount, contextFilterEnabled);
  }, [currentPage, extensions, contextFilterEnabled, slotCount]);

  const hoveredPetal = useMemo(
    () => activePetals.find((p) => p.id === hoveredWedge) ?? null,
    [activePetals, hoveredWedge]
  );

  const wheelSize = size || WHEEL_SIZE_PX.medium;
  const scale = wheelSize / 272;
  const count = activePetals.length;
  const labelFontSize = count <= 4 ? "12.5px" : count <= 7 ? "11px" : "10.5px";

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
          activePetals.length,
          wheelSize / 2,
          wheelSize / 2,
          scale
        );
        const candidate = idx !== null ? activePetals[idx] : null;
        const nextId = candidate ? candidate.id : null;
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
        if (hoveredPetal && files.length > 0) {
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

          {/* Active highlighted petal gradient — Lime logo match */}
          <linearGradient
            id="petal-active-gradient"
            x1="0%"
            y1="0%"
            x2="100%"
            y2="100%"
          >
            <stop offset="0%" stopColor="#d6f224" />
            <stop offset="100%" stopColor="#b2d415" />
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
        {activePetals.map((petal, index) => {
          const isHighlighted = hoveredWedge === petal.id;
          const hasIcon = Boolean(petal.icon);

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
                data-index={index}
                className="preview-petal"
                fill={
                  isHighlighted
                    ? "url(#petal-active-gradient)"
                    : "url(#conversion-fan-glass)"
                }
                stroke={
                  isHighlighted
                    ? "#1e7d23"
                    : "rgba(255, 255, 255, 0.85)"
                }
                strokeWidth={isHighlighted ? 1.5 : 1.2}
                style={{
                  filter: isHighlighted
                    ? "drop-shadow(0 4px 16px rgba(203, 231, 31, 0.55))"
                    : "drop-shadow(0 2px 5px rgba(0,0,0,0.06))",
                  transformOrigin: "0px 0px",
                  transform: isHighlighted ? "scale(1.025)" : "scale(1)",
                  transition:
                    "transform 0.12s cubic-bezier(0.16, 1, 0.3, 1), fill 0.12s ease, stroke 0.12s ease",
                }}
              />

              {/* Tool Icon (if defined) */}
              {hasIcon && petal.icon && (
                <g
                  transform={
                    activePetals.length === 1
                      ? "translate(0, -94) scale(0.66) translate(-12, -12)"
                      : `translate(${petal.labelX}, ${petal.labelY - 9}) scale(0.58) translate(-12, -12)`
                  }
                  style={{
                    pointerEvents: "none",
                    userSelect: "none",
                  }}
                >
                  <ToolIcon
                    type={petal.icon.type}
                    stroke={isHighlighted ? "#163300" : "#222428"}
                  />
                </g>
              )}

              {/* Petal Label */}
              <text
                className="wheel-label preview-format-label preview-tool-label wheel-tool-label"
                x={petal.labelX}
                y={
                  activePetals.length === 1
                    ? -74
                    : hasIcon
                    ? petal.labelY + 11.5
                    : petal.labelY
                }
                textAnchor="middle"
                dominantBaseline="central"
                fill={isHighlighted ? "#163300" : "#222428"}
                fontSize={
                  hasIcon
                    ? activePetals.length === 1
                      ? "12px"
                      : activePetals.length <= 4
                      ? "10px"
                      : "8.5px"
                    : labelFontSize
                }
                fontWeight="700"
                fontFamily="system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
                letterSpacing={hasIcon ? "0.04em" : "0.04em"}
                style={{
                  pointerEvents: "none",
                  userSelect: "none",
                  transition: "fill 0.12s ease",
                }}
              >
                {petal.title}
              </text>

              {/* Sublabel for solitary tools (e.g. TRIM on 1-petal ring, centered in bottom arc) */}
              {petal.action === "trim" && activePetals.length === 1 && (
                <text
                  className="wheel-tool-sublabel"
                  x={0}
                  y={78}
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill={isHighlighted ? "#163300" : "#4b5563"}
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

        {/* Central Hub */}
        <g
          className="center-target"
          style={{ cursor: hasTools ? "pointer" : "default" }}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            if (hasTools) onTogglePage();
          }}
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            if (hasTools) onTogglePage();
          }}
          onWheel={(e) => {
            e.preventDefault();
            e.stopPropagation();
            if (hasTools) onTogglePage();
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            if (hasTools) onTogglePage();
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
              {/* 2 Page Indicator Dots - only shown if tools page exists */}
              {hasTools && (
                <>
                  <circle
                    cx="-4"
                    cy="20"
                    r="1.8"
                    fill={currentPage === "convert" ? "#1e7d23" : "#d1d5db"}
                  />
                  <circle
                    cx="4"
                    cy="20"
                    r="1.8"
                    fill={currentPage === "tools" ? "#1e7d23" : "#d1d5db"}
                  />
                </>
              )}
            </>
          )}
        </g>
      </svg>
    </div>
  );
}

export const RadialWheel = React.memo(RadialWheelInner);
