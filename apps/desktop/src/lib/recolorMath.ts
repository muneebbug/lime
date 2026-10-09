/**
 * The colour maths behind the Recolor Image preview.
 *
 * This is a deliberate, line-for-line port of `crates/wheel-engines/src/recolor.rs`
 * so the live preview and the saved file agree. The window recolours pixels on a
 * canvas for instant feedback and then asks Rust to do the real work at full
 * resolution; if the two implementations disagreed, the preview would be a lie.
 *
 * The one thing that is *not* duplicated is extraction. Counting every distinct
 * colour in a twelve-megapixel photo is too slow to do on the main thread, so
 * Rust returns the list once and the window holds it.
 *
 * Keep both files in step. If you change a conversion here, change it there.
 */

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** A colour as `#RRGGBB`, the format the Rust side sends and expects. */
export type Hex = string;

export interface RecolorRule {
  from: Hex;
  to: Hex;
}

export type RecolorMode = "replace" | "all_to_one";

/** Mirrors `RecolorParams` in the engine. */
export interface RecolorParams {
  mode: RecolorMode;
  rules: RecolorRule[];
  all_to_one: Hex;
  tolerance: number;
  preserve_shading: boolean;
}

/** Mirrors `ExtractedColor` in the engine. */
export interface ExtractedColor {
  color: Hex;
  count: number;
  coverage: number;
}

/** Mirrors `ColorExtraction` in the engine. */
export interface ColorExtraction {
  colors: ExtractedColor[];
  total_unique: number;
  /**
   * Visible pixels considered. Pixels at or below the engine's alpha floor are
   * excluded, so this is smaller than the pixel count of a flat vector shape
   * whose edges are antialiased.
   */
  visible_pixels: number;
  truncated: boolean;
}

/**
 * Mirrors `ColorGroup` in the engine.
 *
 * One of these is what a single replacement rule actually repaints. The
 * extracted list above is a census; this is the edit.
 */
export interface ColorGroup {
  color: Hex;
  /** Distinct colours merged in at the current tolerance. */
  merged: number;
  count: number;
  coverage: number;
}

export const DEFAULT_TOLERANCE = 30;

export function defaultParams(): RecolorParams {
  return {
    mode: "replace",
    rules: [],
    all_to_one: "#CBE71F",
    tolerance: DEFAULT_TOLERANCE,
    // Off by default, matching the Rust default in recolor.rs. "All to one"
    // means paint everything that colour; keeping the shading is the surprising
    // result and has to be asked for.
    preserve_shading: false,
  };
}

/** `#RGB` and `#RRGGBB`, with or without the hash. Returns null when unusable. */
export function parseHex(text: string): Rgb | null {
  const hex = text.trim().replace(/^#/, "").replace(/\s+/g, "");
  const digit = (c: string) => {
    const d = parseInt(c, 16);
    return Number.isNaN(d) || d < 0 || d > 15 ? null : d;
  };

  if (hex.length === 3) {
    const [r, g, b] = [hex[0], hex[1], hex[2]].map(digit);
    return r === null || g === null || b === null
      ? null
      : { r: r * 17, g: g * 17, b: b * 17 };
  }

  if (hex.length === 6) {
    const [r1, r2, g1, g2, b1, b2] = hex.split("").map(digit);
    if ([r1, r2, g1, g2, b1, b2].some((v) => v === null)) return null;
    return {
      r: (r1 as number) * 16 + (r2 as number),
      g: (g1 as number) * 16 + (g2 as number),
      b: (b1 as number) * 16 + (b2 as number),
    };
  }

  return null;
}

/** Uppercase `#RRGGBB`, or null if the text is not a colour. */
export function normalizeHex(text: string): Hex | null {
  const rgb = parseHex(text);
  return rgb ? toHex(rgb) : null;
}

export function toHex({ r, g, b }: Rgb): Hex {
  const pair = (v: number) =>
    Math.max(0, Math.min(255, Math.round(v))).toString(16).toUpperCase().padStart(2, "0");
  return `#${pair(r)}${pair(g)}${pair(b)}`;
}

/** `rgb(...)` / `rgba(...)`, for the few places CSS needs a colour value. */
export function toCssRgb({ r, g, b }: Rgb, alpha = 1): string {
  const clamped = (v: number) => Math.max(0, Math.min(255, Math.round(v)));
  return alpha >= 1
    ? `rgb(${clamped(r)} ${clamped(g)} ${clamped(b)})`
    : `rgba(${clamped(r)}, ${clamped(g)}, ${clamped(b)}, ${alpha})`;
}

function srgbToLinear(channel: number): number {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

function linearToSrgb(value: number): number {
  const c = Math.max(0, Math.min(1, value));
  const encoded = c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
  return Math.round(encoded * 255);
}

interface Lab {
  l: number;
  a: number;
  b: number;
}

function toOklab({ r, g, b }: Rgb): Lab {
  const rl = srgbToLinear(r);
  const gl = srgbToLinear(g);
  const bl = srgbToLinear(b);

  const l = 0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl;
  const m = 0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl;
  const s = 0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl;

  const lc = Math.cbrt(l);
  const mc = Math.cbrt(m);
  const sc = Math.cbrt(s);

  return {
    l: 0.2104542553 * lc + 0.793617785 * mc - 0.0040720468 * sc,
    a: 1.9779984951 * lc - 2.428592205 * mc + 0.4505937099 * sc,
    b: 0.0259040371 * lc + 0.7827717662 * mc - 0.808675766 * sc,
  };
}

function fromOklab({ l, a, b }: Lab): Rgb {
  const lc = l + 0.3963377774 * a + 0.2158037573 * b;
  const mc = l - 0.1055613458 * a - 0.0638541728 * b;
  const sc = l - 0.0894841775 * a - 1.291485548 * b;

  const l3 = lc * lc * lc;
  const m3 = mc * mc * mc;
  const s3 = sc * sc * sc;

  return {
    r: linearToSrgb(4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3),
    g: linearToSrgb(-1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3),
    b: linearToSrgb(-0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3),
  };
}

/**
 * Convert the 0-100 tolerance into an OKLab distance.
 *
 * Non-linear on purpose, matching the engine: the low end of the slider is where
 * the useful precision lives, and a linear scale would spend most of its travel
 * blurring everything together.
 */
export function toleranceToDistance(tolerance: number): number {
  const t = Math.max(0, Math.min(100, tolerance)) / 100;
  return Math.pow(t, 1.5) * 0.5;
}

/**
 * Whether a colour is close enough to a rule's `from` to be replaced.
 *
 * Exported because the eyedropper uses it to warn when the colour someone just
 * clicked will not actually be matched at the current tolerance.
 */
export function matchesRule(color: Rgb, from: Rgb, tolerance: number): boolean {
  const a = toOklab(color);
  const b = toOklab(from);
  const distance = Math.hypot(a.l - b.l, a.a - b.a, a.b - b.b);
  return distance <= toleranceToDistance(tolerance);
}

/**
 * Recolour `data` in place.
 *
 * Operates on a `Uint8ClampedArray` of RGBA bytes, which is what
 * `ImageData.data` and `createImageBitmap` give you. Returns true if anything
 * changed, so callers can skip a repaint.
 *
 * Transparent pixels are skipped for the same reason the engine skips them: the
 * RGB under a fully transparent pixel is leftover data, and recolouring it
 * paints colour into an area the user believes is empty.
 */
export function applyRecolor(data: Uint8ClampedArray, params: RecolorParams): boolean {
  return params.mode === "all_to_one" ? applyAllToOne(data, params) : applyRules(data, params);
}

function applyRules(data: Uint8ClampedArray, params: RecolorParams): boolean {
  // Mirror the engine: first rule for a given `from` wins, so two rows pointing
  // at the same source colour cannot fight over every pixel.
  const seen = new Set<string>();
  const rules: { from: Lab; to: Rgb }[] = [];
  for (const rule of params.rules) {
    const from = parseHex(rule.from);
    const to = parseHex(rule.to);
    if (!from || !to) continue;
    const key = toHex(from);
    if (seen.has(key)) continue;
    seen.add(key);
    rules.push({ from: toOklab(from), to });
  }

  if (rules.length === 0) return false;

  const maxDistance = toleranceToDistance(params.tolerance);
  let changed = false;

  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) continue;

    const lab = toOklab({ r: data[i], g: data[i + 1], b: data[i + 2] });

    let bestDistance = Infinity;
    let bestTarget: Rgb | null = null;

    for (const rule of rules) {
      const distance = Math.hypot(
        lab.l - rule.from.l,
        lab.a - rule.from.a,
        lab.b - rule.from.b
      );
      if (distance <= maxDistance && distance < bestDistance) {
        bestDistance = distance;
        bestTarget = rule.to;
      }
    }

    if (bestTarget) {
      if (
        data[i] !== bestTarget.r ||
        data[i + 1] !== bestTarget.g ||
        data[i + 2] !== bestTarget.b
      ) {
        changed = true;
      }
      data[i] = bestTarget.r;
      data[i + 1] = bestTarget.g;
      data[i + 2] = bestTarget.b;
    }
  }

  return changed;
}

function applyAllToOne(data: Uint8ClampedArray, params: RecolorParams): boolean {
  const target = parseHex(params.all_to_one);
  if (!target) return false;

  if (!params.preserve_shading) {
    let changed = false;
    for (let i = 0; i < data.length; i += 4) {
      if (data[i + 3] === 0) continue;
      if (
        data[i] !== target.r ||
        data[i + 1] !== target.g ||
        data[i + 2] !== target.b
      ) {
        changed = true;
      }
      data[i] = target.r;
      data[i + 1] = target.g;
      data[i + 2] = target.b;
    }
    return changed;
  }

  // Keep each pixel's lightness, take the target's hue and chroma. Flattening a
  // photo to one colour otherwise throws away all of its form.
  const targetLab = toOklab(target);
  let changed = false;

  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) continue;

    const source = toOklab({ r: data[i], g: data[i + 1], b: data[i + 2] });
    const shifted = fromOklab({
      l: Math.max(0, Math.min(1, source.l)),
      a: targetLab.a,
      b: targetLab.b,
    });

    if (
      data[i] !== shifted.r ||
      data[i + 1] !== shifted.g ||
      data[i + 2] !== shifted.b
    ) {
      changed = true;
    }
    data[i] = shifted.r;
    data[i + 1] = shifted.g;
    data[i + 2] = shifted.b;
  }

  return changed;
}

/** True when these params would leave the image alone. */
export function isNoop(params: RecolorParams): boolean {
  return params.mode === "replace" ? params.rules.length === 0 : false;
}
