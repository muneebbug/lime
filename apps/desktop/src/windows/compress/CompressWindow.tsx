import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowDownToLine,
  Check,
  ChevronDown,
  Gauge,
  Image as ImageIcon,
  Loader2,
  Settings2,
  TriangleAlert,
} from "lucide-react";

import { WindowFrame } from "../../ui/WheelUI";

/**
 * Compress Image.
 *
 * Two levels of detail, and the split is deliberate. Basic is the three choices
 * almost everyone makes: how hard, how big, and hit-a-size. Advanced exposes the
 * per-format knobs underneath, which are worth having - progressive JPEG and
 * zopfli are real gains - and which nobody should have to think about by default.
 *
 * Every advanced control reads "inherit" until touched. That is not a detail:
 * a preset is a coherent set of choices, and letting someone change one knob
 * without the others leaves them with a combination the preset never described
 * and this window cannot predict.
 */

export type Preset = "balanced" | "strong" | "maximal";
export type Chroma = "c444" | "c422" | "c420" | "c411" | "c410";
export type TiffAlgorithm = "none" | "lzw" | "deflate" | "packbits";
export type TiffLevel = "fast" | "balanced" | "best";

interface CompressSource {
  path: string;
  file_name: string;
  extension: string;
  size: number;
  width: number;
  height: number;
}

interface CompressSaved {
  output_path: string;
  file_name: string;
  original_size: number;
  output_size: number;
  unchanged: boolean;
  target_missed: boolean;
}

interface Advanced {
  quality?: number;
  chroma_subsampling?: Chroma;
  progressive?: boolean;
  lossless_optimize?: boolean;
  optimize_png?: boolean;
  force_zopfli?: boolean;
  webp_lossless?: boolean;
  tiff_compression?: TiffAlgorithm;
  tiff_deflate_level?: TiffLevel;
  preserve_icc?: boolean;
}

const PRESETS: { id: Preset; label: string; blurb: string }[] = [
  {
    id: "balanced",
    label: "Balanced",
    blurb: "Smaller files with no visible loss. Most people should stop here.",
  },
  {
    id: "strong",
    label: "Strong",
    blurb: "Noticeably smaller. Fine for sharing, not for printing.",
  },
  {
    id: "maximal",
    label: "Maximal",
    blurb:
      "Every lossless gain and the slowest encoders. Minutes on a large PNG, so expect to wait.",
  },
];

/** Longest edge offered as a one-click resize, in pixels. */
const SIZE_CHOICES = [
  { label: "Keep original", value: 0 },
  { label: "3840 px · 4K", value: 3840 },
  { label: "2560 px · QHD", value: 2560 },
  { label: "1920 px · 1080p", value: 1920 },
  { label: "1280 px", value: 1280 },
  { label: "800 px", value: 800 },
];

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function sourceFile(): { path: string; name: string } | null {
  const params = new URLSearchParams(window.location.search);
  const files = params.get("files");
  if (!files) return null;
  try {
    const parsed = JSON.parse(files);
    const first = Array.isArray(parsed) ? parsed[0] : parsed;
    if (typeof first !== "string") return null;
    const name = first.split(/[\\/]/).pop() ?? first;
    return { path: first, name };
  } catch {
    return null;
  }
}

/** True when this preset should warn the user before starting. */
function isSlow(preset: Preset, advanced: Advanced, hasTarget: boolean): boolean {
  // Target-size search encodes repeatedly, so it is slow on any preset.
  if (hasTarget) return true;
  if (advanced.force_zopfli) return true;
  if (preset === "maximal") return true;
  return false;
}

export function CompressWindow() {
  const file = useMemo(sourceFile, []);
  const [source, setSource] = useState<CompressSource | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [preset, setPreset] = useState<Preset>("balanced");
  const [maxDimension, setMaxDimension] = useState(0);
  const [targetEnabled, setTargetEnabled] = useState(false);
  const [targetMb, setTargetMb] = useState(1);
  const [keepMetadata, setKeepMetadata] = useState(false);
  const [advanced, setAdvanced] = useState<Advanced>({});
  const [showAdvanced, setShowAdvanced] = useState(false);

  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<CompressSaved | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    if (!file) {
      setLoadError("No file was given to compress.");
      return;
    }
    let cancelled = false;
    setLoading(true);
    invoke<CompressSource>("compress_inspect", { path: file.path })
      .then((found) => {
        if (!cancelled) setSource(found);
      })
      .catch((e) => {
        if (!cancelled) setLoadError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [file]);

  const isJpeg = source?.extension === "jpg" || source?.extension === "jpeg";
  const isPng = source?.extension === "png";
  const isWebp = source?.extension === "webp";
  const isTiff = source?.extension === "tiff" || source?.extension === "tif";
  const targetBytes = targetEnabled ? Math.round(targetMb * 1024 * 1024) : null;

  /** Live estimate, so the size can be seen before committing to a slow run. */
  const estimate = useMemo(() => {
    if (!source) return null;
    return estimateOutput(
      source.size,
      source.width,
      source.height,
      preset,
      advanced.quality,
      advanced.optimize_png === false,
      isPng,
      maxDimension,
      targetBytes
    );
  }, [
    source,
    preset,
    maxDimension,
    advanced.quality,
    advanced.optimize_png,
    isPng,
    targetBytes,
  ]);

/**
 * A rough "about how big will this end up" for the lossy codecs.
 *
 * Deliberately returns nothing for PNG. Lossless PNG optimisation has a hard
 * ceiling around 10% on a photo-like image - measured at 10.2% on a real
 * warehouse photo - and the only way to get a large PNG reduction is to quantise
 * the palette, which is a different mode rather than a different number on the
 * same scale. A single curve across both would print a confident wrong number,
 * which is worse than printing none.
 */
function estimateOutput(
  sourceSize: number,
  width: number,
  height: number,
  preset: Preset,
  quality: number | undefined,
  quantise: boolean,
  isPng: boolean,
  maxDimension: number,
  targetBytes: number | null
): number | null {
  if (isPng && !quantise) return null;

  // Lossy PNG via imagequant measured at 57% off a real photo; the JPEG and WebP
  // anchors are the usual quality-to-size behaviour.
  const base = quantise
    ? 0.43
    : preset === "balanced"
    ? 0.82
    : preset === "strong"
    ? 0.62
    : 0.42;
  let expected = sourceSize * base;

  if (maxDimension > 0 && width > 0 && height > 0) {
    const longest = Math.max(width, height);
    if (longest > maxDimension) {
      const scale = maxDimension / longest;
      // Area, not a linear factor: halving an edge quarters the pixels.
      expected *= scale * scale;
    }
  }

  if (quality !== undefined) {
    const q = Math.min(100, Math.max(1, quality));
    expected *= q / 80;
  }
  if (targetBytes) {
    expected = Math.min(expected, targetBytes);
  }
  return Math.max(0, Math.round(expected));
}

  const set = useCallback(<K extends keyof Advanced>(key: K, value: Advanced[K]) => {
    setAdvanced((current) => ({ ...current, [key]: value }));
  }, []);

  // `window.close()` in a webview context does not close the native window; it
  // tears down the browsing context and leaves an empty transparent frame on
  // screen. The caption button in WindowFrame gets this right, and the footer's
  // Close has to go through the same API.
  const closeWindow = useCallback(() => {
    getCurrentWebviewWindow()
      .close()
      .catch(console.error);
  }, []);

  /** Back to whatever the preset would have chosen. */
  const resetAdvanced = useCallback(() => {
    setAdvanced({});
  }, []);

  const inherit = useCallback(
    <K extends keyof Advanced>(key: K, describe: string) =>
      advanced[key] === undefined ? (
        <span className="text-[10px] text-text-faint">From preset</span>
      ) : (
        <button
          onClick={() => set(key, undefined)}
          className="text-[10px] text-text-muted hover:text-text-primary transition-colors"
          title={`Reset ${describe} to the preset's value`}
        >
          Reset
        </button>
      ),
    [advanced, set]
  );

  const compress = useCallback(async () => {
    if (!source) return;
    setSaving(true);
    setSaveError(null);
    setSaved(null);
    try {
      const result = await invoke<CompressSaved>("compress_apply", {
        inputPath: source.path,
        params: {
          preset,
          max_dimension: maxDimension > 0 ? maxDimension : null,
          target_size: targetBytes,
          keep_metadata: keepMetadata,
          keep_rotation: true,
          advanced,
        },
      });
      setSaved(result);

      // Close on a real result - the job is done and the notification carries
      // the detail. Stay open when nothing was written: closing there would leave
      // no evidence at all, and "already as small as it gets" is something the
      // user needs to read before they try a stronger setting.
      if (!result.unchanged) {
        closeWindow();
      }
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  }, [source, preset, maxDimension, targetBytes, keepMetadata, advanced, closeWindow]);

  const slow = isSlow(preset, advanced, targetEnabled);

  return (
    <WindowFrame title="Compress" bodyClassName="flex flex-col">
      <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">
        {loadError ? (
          <Notice tone="error">{loadError}</Notice>
        ) : !source ? (
          <div className="flex items-center justify-center gap-2 py-10 text-[12px] text-text-muted">
            <Loader2 size={14} className="animate-spin" />
            Reading file…
          </div>
        ) : (
          <>
            {/* The file, and the whole point of the tool in one row. */}
            <div className="flex items-center gap-3 p-3 rounded-lg bg-surface-raised border border-line-subtle">
              <ImageIcon size={16} className="text-text-muted shrink-0" />
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-medium truncate">{source.file_name}</div>
                <div className="text-[11px] text-text-muted">
                  {formatBytes(source.size)}
                  {source.width > 0 && ` · ${source.width} × ${source.height}`}
                  {` · .${source.extension}`}
                </div>
              </div>
              {saved && (
                <div className="text-right shrink-0">
                  <div className="text-[13px] font-semibold text-accent">
                    {formatBytes(saved.output_size)}
                  </div>
                  <div className="text-[10px] text-text-muted">
                    {saved.unchanged
                      ? "already smallest"
                      : `−${formatBytes(saved.original_size - saved.output_size)}`}
                  </div>
                </div>
              )}
            </div>

            {/* ---- Basic ---- */}
            <section className="space-y-2">
              <Label>
                Compression preset
                <div className="flex gap-1 mt-1.5">
                  {PRESETS.map((p) => (
                    <button
                      key={p.id}
                      onClick={() => setPreset(p.id)}
                      aria-pressed={preset === p.id}
                      className={`flex-1 px-3 py-1.5 text-[12px] font-medium rounded-lg border transition-colors ${
                        preset === p.id
                          ? "bg-accent border-accent text-accent-ink"
                          : "bg-surface-control border-line-strong text-text-secondary hover:bg-surface-control-hover"
                      }`}
                    >
                      {p.label}
                    </button>
                  ))}
                </div>
                <p className="mt-1.5 text-[11px] text-text-muted leading-relaxed">
                  {PRESETS.find((p) => p.id === preset)?.blurb}
                </p>
              </Label>

              <Label>
                Image size
                <div className="flex items-center gap-2 mt-1.5">
                  <select
                    value={maxDimension}
                    onChange={(e) => setMaxDimension(Number(e.target.value))}
                    className="flex-1 px-2.5 py-1.5 text-[12px] rounded-lg bg-surface-control border border-line-strong focus:border-accent focus:outline-none"
                  >
                    {SIZE_CHOICES.map((c) => (
                      <option key={c.value} value={c.value}>
                        {c.label}
                      </option>
                    ))}
                  </select>
                  {maxDimension > 0 && source.width > 0 && (
                    <span className="text-[11px] text-text-muted shrink-0">
                      {Math.round(
                        (Math.min(source.width, source.height) *
                          Math.min(1, maxDimension / Math.max(source.width, source.height)))
                      ).toLocaleString()}{" "}
                      ×{" "}
                      {Math.round(
                        Math.max(source.width, source.height) *
                          Math.min(1, maxDimension / Math.max(source.width, source.height))
                      ).toLocaleString()}
                    </span>
                  )}
                </div>
              </Label>

              <div className="pt-1 border-t border-line-subtle">
                <Toggle
                  checked={targetEnabled}
                  onChange={setTargetEnabled}
                  label="Compress to target file size"
                />
                {targetEnabled && (
                  <div className="mt-2 pl-6 space-y-1.5">
                    <div className="flex items-center gap-2">
                      <input
                        type="number"
                        min={0.01}
                        step={0.1}
                        value={targetMb}
                        onChange={(e) =>
                          setTargetMb(Math.max(0.01, Number(e.target.value) || 0.01))
                        }
                        className="w-24 px-2.5 py-1.5 text-[12px] rounded-lg bg-surface-control border border-line-strong focus:border-accent focus:outline-none"
                      />
                      <span className="text-[11px] text-text-muted">MB or smaller</span>
                      {source.size > 0 && (
                        <span className="text-[11px] text-text-faint">
                          (now {formatBytes(source.size)})
                        </span>
                      )}
                    </div>
                    <p className="text-[10px] text-text-faint leading-relaxed">
                      Lime searches for the smallest file under this size, so the
                      quality slider does not apply — the search picks its own.
                      {isPng && (
                        <>
                          {" "}
                          PNG has no quality dial to turn in a predictable way, so
                          the target may not be reachable. You will be told if it
                          is not.
                        </>
                      )}
                    </p>
                  </div>
                )}
              </div>

              <div className="pt-1 border-t border-line-subtle">
                <Toggle
                  checked={keepMetadata}
                  onChange={setKeepMetadata}
                  label="Keep EXIF and other metadata"
                />
                <p className="mt-1 pl-6 text-[10px] text-text-faint leading-relaxed">
                  Phone photos often carry 10–50 KB of it. Image orientation is
                  always kept either way, so nothing comes out rotated.
                </p>
              </div>
            </section>

            {/* ---- Advanced ---- */}
            <section className="border-t border-line-subtle pt-3">
              <button
                onClick={() => setShowAdvanced((v) => !v)}
                className="flex items-center gap-2 w-full text-left"
                aria-expanded={showAdvanced}
              >
                <Settings2 size={13} className="text-text-muted" />
                <span className="text-[12px] font-semibold uppercase tracking-wider text-text-muted">
                  Advanced
                </span>
                <span className="text-[11px] text-text-faint">
                  {Object.keys(advanced).length === 0
                    ? "preset defaults"
                    : `${Object.keys(advanced).length} changed`}
                </span>
                <ChevronDown
                  size={14}
                  className={`ml-auto text-text-faint transition-transform ${
                    showAdvanced ? "rotate-180" : ""
                  }`}
                />
              </button>

              {showAdvanced && (
                <div className="mt-3 space-y-4">
                  <p className="text-[11px] text-text-muted leading-relaxed">
                    These override the preset for this run only. Anything left
                    alone keeps the preset's value, so you can change one thing
                    and still know what the others are.
                  </p>

                  {/* Quality applies to whichever lossy codec the file uses. */}
                  <Slider
                    label="Quality"
                    hint={
                      isPng
                        ? "For PNG this reduces the colour palette, not the pixels. Leave it alone unless the image is photo-like."
                        : "Lower is smaller. The scale is perceptual, not a percentage of data."
                    }
                    value={advanced.quality}
                    inherit={inherit("quality", "quality")}
                    onChange={(v) => set("quality", v)}
                  />

                  {isJpeg && (
                    <>
                      <Choice
                        label="Chroma subsampling"
                        inherit={inherit("chroma_subsampling", "subsampling")}
                        value={advanced.chroma_subsampling ?? "c420"}
                        onChange={(v) => set("chroma_subsampling", v)}
                        options={[
                          { value: "c444", label: "4:4:4 · no loss" },
                          { value: "c422", label: "4:2:2" },
                          { value: "c420", label: "4:2:0 · standard" },
                          { value: "c411", label: "4:1:1" },
                          { value: "c410", label: "4:1:0 · smallest" },
                        ]}
                      />
                      <Toggle
                        inherit={inherit("progressive", "progressive")}
                        checked={advanced.progressive ?? true}
                        onChange={(v) => set("progressive", v)}
                        label="Progressive JPEG"
                        hint="Loads top to bottom. Slightly smaller at the same quality."
                      />
                      <Toggle
                        inherit={inherit("lossless_optimize", "lossless optimisation")}
                        checked={advanced.lossless_optimize ?? false}
                        onChange={(v) => set("lossless_optimize", v)}
                        label="Lossless re-compression"
                        hint="Rebuilds the Huffman tables without touching a pixel. Usually about 10% off and the image is identical — but it ignores the quality slider, because there is nothing to quantise."
                      />
                    </>
                  )}

{isPng && (
                      <>
                        <Toggle
                          inherit={inherit("optimize_png", "PNG mode")}
                          checked={advanced.optimize_png ?? true}
                          onChange={(v) => set("optimize_png", v)}
                          label="Lossless — keep every colour"
                          hint="Re-filters the data and rebuilds the tables. Nothing changes visibly. This is all a photo-like PNG has to offer: measured at about 10% off."
                        />
                        <Toggle
                          inherit={inherit("optimize_png", "PNG mode")}
                          checked={advanced.optimize_png === false}
                          onChange={(v) => set("optimize_png", !v)}
                          label="Lossy — reduce to 256 colours"
                          hint="Much smaller, and genuinely lossy. Measured on a 1.4 MB warehouse photo: 1.28 MB lossless versus 0.62 MB quantised. Gradients band; flat brand colours survive."
                        />
                        <Toggle
                          inherit={inherit("force_zopfli", "zopfli")}
                          checked={advanced.force_zopfli ?? false}
                          onChange={(v) => set("force_zopfli", v)}
                          label="Use zopfli"
                          hint="The smallest PNG a lossless encoder can produce, and slow enough to think about. Measured on that same file: 75 seconds for 0.7 percentage points."
                        />
                      </>
                    )}

                  {isWebp && (
                    <Toggle
                      inherit={inherit("webp_lossless", "lossless")}
                      checked={advanced.webp_lossless ?? false}
                      onChange={(v) => set("webp_lossless", v)}
                      label="Lossless WebP"
                      hint="Usually makes the file bigger, since WebP already compresses well. Try it if your original was a PNG."
                    />
                  )}

                  {isTiff && (
                    <>
                      <Choice
                        label="TIFF compression"
                        inherit={inherit("tiff_compression", "TIFF compression")}
                        value={advanced.tiff_compression ?? "deflate"}
                        onChange={(v) => set("tiff_compression", v)}
                        options={[
                          { value: "deflate", label: "Deflate · best balance" },
                          { value: "lzw", label: "LZW" },
                          { value: "packbits", label: "PackBits" },
                          { value: "none", label: "None · uncompressed" },
                        ]}
                      />
                      <Choice
                        label="Deflate effort"
                        inherit={inherit("tiff_deflate_level", "effort")}
                        value={advanced.tiff_deflate_level ?? "balanced"}
                        onChange={(v) => set("tiff_deflate_level", v)}
                        options={[
                          { value: "fast", label: "Fast" },
                          { value: "balanced", label: "Balanced" },
                          { value: "best", label: "Best" },
                        ]}
                      />
                    </>
                  )}

                  <Toggle
                    inherit={inherit("preserve_icc", "ICC profile")}
                    checked={advanced.preserve_icc ?? keepMetadata}
                    onChange={(v) => set("preserve_icc", v)}
                    label="Keep colour profile"
                    hint="Dropping it changes how the image looks on a wide-gamut screen."
                  />

                  <button
                    onClick={resetAdvanced}
                    disabled={Object.keys(advanced).length === 0}
                    className="px-2.5 py-1 text-[11px] rounded-md bg-surface-control hover:bg-surface-control-hover border border-line-strong text-text-secondary disabled:opacity-40 transition-colors"
                  >
                    Reset all to preset
                  </button>
                </div>
              )}
            </section>

            {!saved &&
              (estimate !== null ? (
                <Notice tone="info">
                  <div className="flex items-center gap-2">
                    <Gauge size={13} className="shrink-0" />
                    <span>
                      Expect around {formatBytes(estimate)}
                      {source.size > 0 && (
                        <>
                          {" "}
                          · roughly{" "}
                          {Math.max(
                            0,
                            Math.round((1 - estimate / source.size) * 100)
                          )}
                          % smaller
                        </>
                      )}
                      . An estimate, not a promise.
                    </span>
                  </div>
                </Notice>
              ) : isPng && advanced.optimize_png !== false ? (
                // No honest curve exists for lossless PNG, so say the one thing
                // that is reliably true instead of inventing a number.
                <Notice tone="info">
                  <div className="flex items-center gap-2">
                    <Gauge size={13} className="shrink-0" />
                    <span>
                      No size estimate for lossless PNG — it depends entirely on
                      the image. Expect single-digit percentages on a photo; much
                      more on flat art. Resizing is the reliable lever here.
                    </span>
                  </div>
                </Notice>
              ) : null)}

            {slow && !saved && (
              <Notice tone="warn">
                <div className="flex items-start gap-2">
                  <TriangleAlert size={13} className="shrink-0 mt-px" />
                  <span>
                    This setting is slow, sometimes minutes on a large image. Keep
                    the app open.
                  </span>
                </div>
              </Notice>
            )}

            {saveError && <Notice tone="error">{saveError}</Notice>}

            {saved && (
              <Notice tone={saved.unchanged ? "info" : "success"}>
                <div className="flex items-start gap-2">
                  <Check size={13} className="shrink-0 mt-px" />
                  <span>
                    {saved.unchanged ? (
                      <>Already as small as this encoder can make it.</>
                    ) : (
                      <>
                        Saved as{" "}
                        <span className="font-medium">{saved.file_name}</span>{" "}
                        at {formatBytes(saved.output_size)}, down from{" "}
                        {formatBytes(saved.original_size)}.
                        {saved.target_missed && (
                          <>
                            {" "}
                            <strong>The target size was not reached</strong> — this
                            is the smallest achievable at this quality.
                          </>
                        )}
                      </>
                    )}
                  </span>
                </div>
              </Notice>
            )}
          </>
        )}
      </div>

      <footer className="flex items-center justify-end gap-2 px-5 h-16 border-t border-line-subtle shrink-0">
        <button
          onClick={closeWindow}
          className="px-4 py-2 text-[12px] font-medium rounded-lg border border-line-strong bg-surface-control text-text-secondary hover:bg-surface-control-hover transition-colors"
        >
          Close
        </button>
        <button
          onClick={compress}
          disabled={!source || saving || loading}
          className="inline-flex items-center gap-2 px-4 py-2 text-[12px] font-semibold rounded-lg bg-accent text-accent-ink hover:bg-accent-hover disabled:opacity-40 transition-colors"
        >
          {saving ? (
            <>
              <Loader2 size={13} className="animate-spin" />
              Compressing…
            </>
          ) : (
            <>
              <ArrowDownToLine size={13} />
              Compress
            </>
          )}
        </button>
      </footer>
    </WindowFrame>
  );
}

function Label({ children }: { children: React.ReactNode }) {
  return (
    <label className="block text-[12px] font-medium text-text-secondary">
      {children}
    </label>
  );
}

type Tone = "info" | "error" | "warn" | "success";

function Notice({
  tone,
  children,
}: {
  tone: Tone;
  children: React.ReactNode;
}) {
  const styles: Record<Tone, string> = {
    info: "bg-surface-raised border-line-subtle text-text-secondary",
    success: "bg-accent-soft border-accent/25 text-text-secondary",
    warn: "bg-warning-soft border-warning/25 text-text-secondary",
    error: "bg-negative-soft border-negative/25 text-text-secondary",
  };
  return (
    <div
      className={`rounded-lg border p-2.5 text-[11px] leading-relaxed ${styles[tone]}`}
    >
      {children}
    </div>
  );
}

function Toggle({
  checked,
  onChange,
  label,
  hint,
  inherit,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  hint?: string;
  inherit?: React.ReactNode;
}) {
  return (
    <div className="py-0.5">
      <div className="flex items-center gap-2.5">
        <button
          role="checkbox"
          aria-checked={checked}
          onClick={() => onChange(!checked)}
          className={`w-4 h-4 shrink-0 rounded border flex items-center justify-center transition-colors ${
            checked
              ? "bg-accent border-accent text-accent-ink"
              : "border-line-strong bg-surface-control"
          }`}
        >
          {checked && <Check size={11} strokeWidth={3} />}
        </button>
        <span className="text-[12px] text-text-secondary">{label}</span>
        {inherit && <span className="ml-auto">{inherit}</span>}
      </div>
      {hint && (
        <p className="mt-0.5 ml-6.5 text-[10px] text-text-faint leading-relaxed">
          {hint}
        </p>
      )}
    </div>
  );
}

function Slider({
  label,
  hint,
  value,
  onChange,
  inherit,
}: {
  label: string;
  hint?: string;
  value?: number;
  onChange: (v: number | undefined) => void;
  inherit?: React.ReactNode;
}) {
  return (
    <div className="py-0.5">
      <div className="flex items-center gap-2.5">
        <span className="text-[12px] text-text-secondary">{label}</span>
        {inherit && <span className="ml-auto">{inherit}</span>}
      </div>
      <div className="flex items-center gap-2.5 mt-1.5">
        <input
          type="range"
          min={10}
          max={100}
          step={1}
          value={value ?? 80}
          onChange={(e) => onChange(Number(e.target.value))}
          className="flex-1 accent-[var(--color-accent)]"
        />
        <span className="text-[11px] text-text-muted tabular-nums w-14 text-right">
          {value ?? "preset"}
        </span>
      </div>
      {hint && (
        <p className="mt-1 text-[10px] text-text-faint leading-relaxed">{hint}</p>
      )}
    </div>
  );
}

function Choice<T extends string>({
  label,
  value,
  onChange,
  options,
  inherit,
}: {
  label: string;
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
  inherit?: React.ReactNode;
}) {
  return (
    <div className="py-0.5">
      <div className="flex items-center gap-2.5">
        <span className="text-[12px] text-text-secondary">{label}</span>
        {inherit && <span className="ml-auto">{inherit}</span>}
      </div>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as T)}
        className="w-full mt-1.5 px-2.5 py-1.5 text-[12px] rounded-lg bg-surface-control border border-line-strong focus:border-accent focus:outline-none"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}