import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  Check,
  ChevronDown,
  Copy,
  Eye,
  EyeOff,
  ImageOff,
  Info,
  Layers,
  List,
  Loader2,
  Palette,
  Pipette,
  Plus,
  RotateCcw,
Save,
  Search,
  Sparkles,
  Trash2,
  Wand2,
  X,
} from "lucide-react";
import { motion } from "motion/react";
import { WindowFrame, ToggleSwitch } from "../../ui/WheelUI";
import {
  applyRecolor,
  isNoop,
  matchesRule,
  normalizeHex,
  parseHex,
  toCssRgb,
type ColorExtraction,
  type ColorGroup,
  type ExtractedColor,
  type Hex,
  type RecolorMode,
  type RecolorParams,
  type RecolorRule,
} from "../../lib/recolorMath";

/**
 * Recolor Image.
 *
 * One image, two previews, a list of colour replacements. The design goal is
 * that the common case - "make the blue bird red" - takes three actions: pick
 * the colour off the image, pick the new colour, save.
 *
 * Everything happens on a canvas in this window so the preview updates as the
 * tolerance moves. The saved file is produced by the Rust engine instead, so what
 * is shown and what is written both come from the same maths (`lib/recolorMath`
 * is a port of the engine, and the two have to agree).
 */

/** What Rust tells us about the file being edited. */
/**
 * What the backend reports when a file is opened.
 *
 * No colour list here on purpose: counting is a full pass over every pixel, and
 * putting it on the open path meant staring at a blank window for work most
 * sessions never look at. It arrives later, from `recolor_extract_colors`.
 */
interface RecolorSource {
  path: string;
  file_name: string;
  width: number;
  height: number;
  has_transparency: boolean;
  file_size: number;
  extension: string;
  data_url: string;
  /** True when the source is SVG, so vector output can be offered. */
  is_svg: boolean;
}

/** What Rust tells us after a save. */
interface RecolorSaved {
  output_path: string;
  file_name: string;
  /** Zero for an SVG output, which has no intrinsic pixel size. */
  width: number;
  height: number;
  unchanged: boolean;
  /** True when the file was written by editing SVG markup rather than pixels. */
  is_svg: boolean;
  /** How many colour literals the SVG rewrite changed. Zero for raster. */
  replaced_literals: number;
}

/** A rule the user is editing, with stable identity for React keys. */
interface DraftRule extends RecolorRule {
  id: string;
  enabled: boolean;
}

/**
 * Preview cap.
 *
 * A twelve-megapixel photo has sixteen million pixels; recolouring all of them
 * through OKLab on the main thread would drop frames. The preview works on a
 * downscaled copy - still large enough to judge a colour change - while the save
 * always runs at full resolution. The factor is a power of two so the scaling
 * stays cheap.
 */
const PREVIEW_MAX_EDGE = 1100;

/** Colour list length before "show more" appears. */
const COLORS_PAGE = 120;

/**
 * How long to wait after the last change before repainting the preview.
 *
 * Recolouring is a full pass over the pixel buffer in OKLab, which is expensive
 * enough that dragging the tolerance slider - a value that fires on every pixel
 * of movement - queues dozens of passes and leaves the preview lagging behind the
 * thumb. Waiting for a pause makes one pass happen instead of many, and the
 * preview always lands on the value the slider was finally left at.
 *
 * Long enough to coalesce a drag, short enough that letting go feels responsive.
 */
const PREVIEW_DEBOUNCE_MS = 500;

/**
 * Formats offered on save. `""` keeps the source format.
 *
 * SVG is not in this list. It is only reachable by editing a document's own
 * markup, so offering it for a raster source would be offering something the
 * tool cannot do - and failing on save with a confusing error when the user took
 * it up. A PNG cannot become vector output without tracing it into paths, which
 * is a different feature, not a format choice.
 */
const RASTER_FORMATS: { value: string; label: string }[] = [
  { value: "", label: "Same as source" },
  { value: "png", label: "PNG" },
  { value: "jpg", label: "JPEG" },
  { value: "webp", label: "WebP" },
  { value: "bmp", label: "BMP" },
  { value: "tiff", label: "TIFF" },
];

/** Added only when the source is itself an SVG. */
const SVG_FORMAT = { value: "svg", label: "SVG (vector)" };

/** Formats that cannot store an alpha channel, for the save warning. */
const OPAQUE_ONLY = new Set(["jpg", "bmp"]);

/** Extensions that hold SVG markup. */
const SVG_EXTENSIONS = new Set(["svg", "svgz"]);

function isSvgExtension(extension: string | undefined | null): boolean {
  return SVG_EXTENSIONS.has((extension ?? "").toLowerCase());
}

let nextRuleId = 0;
const makeRuleId = () => `rule-${++nextRuleId}`;

export function RecolorWindow() {
  const params = new URLSearchParams(window.location.search);
  const initialPath = useMemo(() => {
    try {
      const files = JSON.parse(params.get("files") ?? "[]") as string[];
      return files[0] ?? "";
    } catch {
      return "";
    }
  }, [params]);

  const [source, setSource] = useState<RecolorSource | null>(null);
  const [loading, setLoading] = useState(Boolean(initialPath));
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<RecolorSaved | null>(null);

  const [rules, setRules] = useState<DraftRule[]>([]);
  const [mode, setMode] = useState<RecolorMode>("replace");
  const [tolerance, setTolerance] = useState(30);
  const [allToOne, setAllToOne] = useState<Hex>("#CBE71F");
  // Off by default, matching the Rust default. "All to one" is the literal
  // request: paint everything this colour. Preserving shading is the surprising
  // result that has to be asked for, and a flat fill is the predictable one.
  const [preserveShading, setPreserveShading] = useState(false);
  const [showOriginal, setShowOriginal] = useState(false);
  const [livePreview, setLivePreview] = useState(true);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [format, setFormat] = useState("");
  const [colorLimit, setColorLimit] = useState(COLORS_PAGE);
  const [zoom, setZoom] = useState(1);

  /**
   * The extracted colours, loaded only when the user asks for them.
   *
   * Counting colours is a full pass over every pixel - 110ms on a one-megapixel
   * image, more on a large photo - and it used to happen during open, so the
   * window sat blank for work most sessions never look at. `null` means "not asked
   * for yet", which is deliberately distinct from "asked for and there were none".
   */
  const [colors, setColors] = useState<ColorExtraction | null>(null);
  const [colorsComplete, setColorsComplete] = useState(true);
  const [colorsLoading, setColorsLoading] = useState(false);
  const [colorsError, setColorsError] = useState<string | null>(null);

  /**
   * Guards against a duplicate request.
   *
   * React 18 mounts effects twice in development StrictMode, and two clicks on
   * "Auto add all colours" can land before the first returns. Both would cost a
   * full pixel pass, and the second would race the first to set state.
   */
  const colorsRequestRef = useRef<Promise<ColorExtraction | null> | null>(null);

  /**
   * The extracted list folded into the swatches a rule would actually repaint.
   *
   * Kept apart from `colors` because the two answer different questions. The
   * extraction is a census and never changes; the groups move with the tolerance
   * slider, because a rule is a sphere rather than a point. Showing the census
   * next to a spherical edit is what made the list describe something the user
   * was not about to get - at the default tolerance one swatch was quietly
   * repainting 316 of the 567 colours listed beside it.
   */
  const [groups, setGroups] = useState<ColorGroup[] | null>(null);

  /**
   * Counts the stale groupings that lost a race.
   *
   * Dragging the slider fires a command per step and they can settle out of
   * order. Without this the list would sometimes show the groups for a
   * tolerance the user has already moved past.
   */
  const groupRequestRef = useRef(0);

  /**
   * Bumped whenever the user undoes their work, so the preview can be repainted
   * without waiting out the debounce.
   *
   * A counter rather than a boolean because several undos in a row must each
   * count: `true -> true` would not re-trigger the effect, and the second Reset
   * would then sit debounced.
   */
  const [undoSignal, setUndoSignal] = useState(0);

  // Untouched pixels, so the preview can always be recomputed from scratch
  // rather than accumulated onto. Accumulating would drift: every edit would
  // build on the last one, and "reset" would have nothing to go back to.
  const originalRef = useRef<ImageData | null>(null);
  const previewCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const originalCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const previewCanvas2dRef = useRef<CanvasRenderingContext2D | null>(null);
  const workRef = useRef<ImageData | null>(null);
  const viewportRef = useRef<HTMLDivElement | null>(null);

  /**
   * Bumped once the source has been decoded into `originalRef`.
   *
   * The buffer itself lives in a ref, because it is large and mutated on every
   * keystroke of the tolerance slider and must never sit in state. But something
   * has to tell React it exists: `image.onload` is async, so the render that
   * sets `source` happens before there are pixels, and neither the sizing maths
   * nor the first repaint can read the ref yet.
   */
  const [bufferVersion, setBufferVersion] = useState(0);

  /**
   * Which `bufferVersion` the preview canvas is currently showing.
   *
   * Lets the repaint below tell "a new image just loaded" (paint at once) from
   * "an edit to the image already on screen" (debounce). A ref rather than state
   * because it is bookkeeping for the effect, not something to render.
   */
  const paintedBufferRef = useRef(0);

  /** The `showOriginal` value the canvas is currently showing. See
   *  `paintedBufferRef`; same purpose, for the before/after toggle. */
  const paintedToggleRef = useRef(false);

  /** The decoded source buffer, held in state only so the panes get a repaint
   *  when it appears. The mutable copy lives in `workRef`. */
  const [previewBuffer, setPreviewBuffer] = useState<ImageData | null>(null);

  /**
   * Size of the preview viewport, so the panes can be scaled to fit whatever
   * room is left after the side panel. Measured rather than assumed, because the
   * panel is a fixed width but the window is resizable and the panes sit side by
   * side.
   */
  const [viewport, setViewport] = useState({ width: 0, height: 0 });

  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return;

    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setViewport({ width, height });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const activeRules = useMemo(
    () => rules.filter((rule) => rule.enabled),
    [rules]
  );

  const engineParams: RecolorParams = useMemo(
    () => ({
      mode,
      rules: activeRules.map(({ from, to }) => ({ from, to })),
      all_to_one: allToOne,
      tolerance,
      preserve_shading: preserveShading,
    }),
    [mode, activeRules, allToOne, tolerance, preserveShading]
  );

  /**
   * The same options, ready to send.
   *
   * `engineParams` is a `useMemo` so the preview effect only runs when a value
   * really changed. Reusing it as the request payload keeps preview and save from
   * ever being fed different options, which is the failure mode that would make
   * the window lie.
   */
  const payload = engineParams;

  const willChangeAnything = useMemo(
    () => !isNoop(engineParams) && livePreview,
    [engineParams, livePreview]
  );

  /**
   * CSS size for each preview canvas.
   *
   * The panes share the available box - side by side when there is room, stacked
   * when there is not - and the image's aspect ratio decides how much of that it
   * fills. The user's zoom multiplies the result.
   *
   * The scale is applied to the *element*, not the backing buffer, so the canvas
   * stays at preview resolution and the browser interpolates. That is what keeps
   * zooming to 400% smooth instead of blocky.
   */
  const { displaySize, stacked } = useMemo(() => {
    if (!previewBuffer || viewport.width === 0) {
      return { displaySize: { width: 0, height: 0 }, stacked: false };
    }

    const gap = 12;
    const padding = 32;

    // Below this, two panes side by side would each be too small to judge a
    // colour against, so they stack instead.
    const stack = viewport.width < 620;

    const boxWidth = stack
      ? Math.max(80, viewport.width - padding)
      : Math.max(80, (viewport.width - gap - padding) / 2);
    const boxHeight = stack
      ? Math.max(80, (viewport.height - gap - padding) / 2)
      : Math.max(80, viewport.height - padding);

    const fit = Math.min(
      boxWidth / previewBuffer.width,
      boxHeight / previewBuffer.height
    );
    const scale = Math.max(0.05, fit * zoom);

    return {
      stacked: stack,
      displaySize: {
        width: Math.max(1, Math.round(previewBuffer.width * scale)),
        height: Math.max(1, Math.round(previewBuffer.height * scale)),
      },
    };
  }, [viewport, zoom, previewBuffer]);

  /** Load a file and reset every edit, since they applied to the old one. */
  const inspect = useCallback(async (path: string) => {
    if (!path) return;
    setLoading(true);
    setError(null);
    setSaved(null);
    // Colours belong to the old file and were never loaded for this one.
    setColors(null);
    setColorsError(null);
    colorsRequestRef.current = null;

    try {
      setSource(await invoke<RecolorSource>("recolor_inspect", { path }));
    } catch (e) {
      setSource(null);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  /**
   * Count the file's colours, once, and remember the result.
   *
   * Cached because the answer cannot change while the file is open, and both
   * entry points - the panel and "Auto add all colours" - want the same list.
   */
  const loadColors = useCallback(async (): Promise<ColorExtraction | null> => {
    if (!source) return null;
    if (colors) return colors;
    if (colorsRequestRef.current) return colorsRequestRef.current;

    setColorsLoading(true);
    setColorsError(null);

    const request = invoke<{ colors: ColorExtraction; complete: boolean }>(
      "recolor_extract_colors",
      { path: source.path, colorLimit: 2000 }
    )
      .then((result) => {
        setColors(result.colors);
        setColorsComplete(result.complete);
        return result.colors;
      })
      .catch((e) => {
        setColorsError(String(e));
        return null;
      })
      .finally(() => {
        setColorsLoading(false);
        colorsRequestRef.current = null;
      });

    colorsRequestRef.current = request;
    return request;
  }, [source, colors]);

  // First expansion of the panel is a clear intent, so load then rather than
  // making the user ask twice.
  useEffect(() => {
if (paletteOpen && !colors && !colorsLoading) void loadColors();
  }, [paletteOpen, colors, colorsLoading, loadColors]);

  /**
   * Regroup the extracted list whenever the tolerance moves.
   *
   * This is a cheap call - a few hundred float comparisons on a list already in
   * memory - so it can follow the slider directly instead of being debounced
   * behind the pixel scan. Doing the grouping here rather than in the window is
   * the point: it has to use the same radius the recolour will use, or the list
   * starts describing an edit the tool is not going to make.
   */
  useEffect(() => {
    if (!colors) {
      setGroups(null);
      return;
    }
    // Nothing to group, so skip the round trip rather than asking for nothing.
    if (colors.colors.length === 0) {
      setGroups([]);
      return;
    }

    const ticket = ++groupRequestRef.current;
    let cancelled = false;

    void invoke<ColorGroup[]>("recolor_group_colors", {
      colors: colors.colors,
      tolerance,
    })
      .then((result) => {
        if (cancelled || ticket !== groupRequestRef.current) return;
        setGroups(result);
      })
      .catch(() => {
        // A failed grouping leaves the panel on the last good list rather than
        // emptying it; the census is still there and still usable.
        if (cancelled || ticket !== groupRequestRef.current) return;
        setGroups(null);
      });

    return () => {
      cancelled = true;
    };
  }, [colors, tolerance]);

  useEffect(() => {
    if (initialPath) void inspect(initialPath);
  }, [initialPath, inspect]);

  /**
   * Decode the file into an offscreen copy the preview can recolour.
   *
   * The file arrives as a data URL so the webview needs no filesystem access.
   * An `Image` is used rather than `createImageBitmap` because bitmap decoding
   * fails outright on some WebP and AVIF builds in the WebView2 runtime, and a
   * thrown rejection here would leave the window with no image and no message.
   */
  useEffect(() => {
    if (!source) {
      originalRef.current = null;
      setPreviewBuffer(null);
      return;
    }

    // Drop the previous file's pixels before the new ones arrive, so a slow
    // decode cannot leave the old image on screen next to the new file name.
    originalRef.current = null;
    setPreviewBuffer(null);

    let cancelled = false;
    const image = new Image();

    image.onload = () => {
      if (cancelled) return;

      const scale = Math.min(
        1,
        PREVIEW_MAX_EDGE / Math.max(image.naturalWidth, image.naturalHeight)
      );
      const width = Math.max(1, Math.round(image.naturalWidth * scale));
      const height = Math.max(1, Math.round(image.naturalHeight * scale));

      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (!context) return;

      context.drawImage(image, 0, 0, width, height);
      const pixels = context.getImageData(0, 0, width, height);
      if (cancelled) return;

      originalRef.current = pixels;

      // Reuse the visible canvases rather than rendering two <img> tags: both
      // panes then draw from the same buffer and cannot drift apart.
      for (const ref of [previewCanvasRef, originalCanvasRef]) {
        const element = ref.current;
        if (!element) continue;
        element.width = width;
        element.height = height;
        ref === previewCanvasRef
          ? (previewCanvas2dRef.current = element.getContext("2d"))
          : element.getContext("2d")?.drawImage(canvas, 0, 0);
      }

      workRef.current = new ImageData(
        new Uint8ClampedArray(pixels.data),
        width,
        height
      );
      // Announce the buffer so the sizing maths and the first repaint run.
      setPreviewBuffer(pixels);
      setBufferVersion((v) => v + 1);
    };

    image.onerror = () => {
      if (!cancelled) setError("That file could not be displayed.");
    };

    image.src = source.data_url;
    return () => {
      cancelled = true;
    };
  }, [source]);

  /**
   * Repaint the preview.
   *
   * Always starts from the pristine buffer rather than the previous result, so
   * dragging the tolerance never compounds and the slider is reversible. When
   * live preview is off the pane shows the original instead, which is how
   * "before and after" is checked without a second window.
   *
   * Debounced, because the work is not a pure function of state - it mutates a
   * canvas - so it needs an explicit teardown rather than a hook. Every re-run
   * clears the pending timer, which is what coalesces a drag into one pass and
   * stops a queued repaint from landing on a file that has since been replaced.
   */
  useEffect(() => {
    const repaint = () => {
      const context = previewCanvas2dRef.current;
      const original = originalRef.current;
      const work = workRef.current;
      if (!context || !original || !work) return;

      // `showOriginal` puts the untouched image back in the recoloured pane,
      // which is the fastest way to answer "is that actually different?" without
      // moving the mouse or losing the edit.
      if (showOriginal) {
        context.putImageData(original, 0, 0);
        return;
      }

      /*
       * Restore first, then recolour.
       *
       * The restore has to be unconditional. Doing it only when there is
       * something to recolour left the buffer holding the previous pass's pixels
       * whenever there was nothing to do - so clearing the rule list (Reset, or
       * deleting the last row) painted the old colours back on screen instead of
       * the original, and switching live preview off did the same. Nothing here
       * can be stale, because the source of truth is always the pristine buffer.
       */
      const target = work.data;
      target.set(original.data);
      if (willChangeAnything) {
        applyRecolor(target, engineParams);
      }

      context.putImageData(
        new ImageData(target, original.width, original.height),
        0,
        0
      );
    };

    /*
     * Repaint immediately, without waiting out the debounce, when:
     *
     *  - a newly decoded image arrived - nothing can be stale, and debouncing
     *    would leave the pane blank for half a second every time a file opens;
     *  - the user undid their work - they asked for the change to be undone, and
     *    holding the previous edit on screen afterwards reads as the button being
     *    broken;
     *  - the Original/Recolored toggle moved - a direct command, like undo.
     *
     * Only edits to an image already on screen get waited on, which is the whole
     * point of the debounce.
     */
    const originalToggleMoved = showOriginal !== paintedToggleRef.current;
    if (
      bufferVersion !== paintedBufferRef.current ||
      undoSignal > 0 ||
      originalToggleMoved
    ) {
      paintedBufferRef.current = bufferVersion;
      paintedToggleRef.current = showOriginal;
      repaint();
      return;
    }

    const timer = window.setTimeout(repaint, PREVIEW_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [engineParams, willChangeAnything, showOriginal, bufferVersion, undoSignal]);

  const chooseFile = useCallback(async () => {
    const picked = await openDialog({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff", "gif", "avif", "ico", "svg", "heic"],
        },
      ],
    });
    if (typeof picked === "string") {
      setRules([]);
      setSaved(null);
      setColorLimit(COLORS_PAGE);
      void inspect(picked);
    }
  }, [inspect]);

  /**
   * Pick a colour straight off either preview.
   *
   * Reads the *original* buffer even when clicking the recoloured pane: the user
   * is identifying a colour that exists in the file, not one the tool produced.
   */
  const pickFromImage = useCallback(
    (which: "original" | "preview", event: React.MouseEvent<HTMLCanvasElement>) => {
      const element = which === "original" ? originalCanvasRef.current : previewCanvasRef.current;
      const original = originalRef.current;
      if (!element || !original) return;

      const bounds = element.getBoundingClientRect();
      const scaleX = element.width / bounds.width;
      const scaleY = element.height / bounds.height;
      const x = Math.floor((event.clientX - bounds.left) * scaleX);
      const y = Math.floor((event.clientY - bounds.top) * scaleY);

      if (x < 0 || y < 0 || x >= element.width || y >= element.height) return;

      const offset = (y * original.width + x) * 4;
      const alpha = original.data[offset + 3];
      if (alpha === 0) {
        setError("That spot is fully transparent, so there is no colour to copy.");
        return;
      }

      const hex = `#${[0, 1, 2]
        .map((i) => original.data[offset + i].toString(16).toUpperCase().padStart(2, "0"))
        .join("")}`;

      setPickerOpen(false);
      setRules((current) => {
        const existing = current.find((rule) => rule.from === hex);
        if (existing) {
          // Already listed: turn it on rather than adding a duplicate row that
          // would compete with the first one for every matching pixel.
          return current.map((rule) =>
            rule.id === existing.id ? { ...rule, enabled: true } : rule
          );
        }
        return [...current, { id: makeRuleId(), from: hex, to: hex, enabled: true }];
      });
    },
    []
  );

  const addManualRule = useCallback(() => {
    setPickerOpen(false);
    setRules((current) => [
      ...current,
      { id: makeRuleId(), from: "#FFFFFF", to: "#FF0000", enabled: true },
    ]);
  }, []);

  const updateRule = useCallback((id: string, patch: Partial<DraftRule>) => {
    setRules((current) =>
      current.map((rule) => (rule.id === id ? { ...rule, ...patch } : rule))
    );
  }, []);

  const removeRule = useCallback((id: string) => {
    setRules((current) => current.filter((rule) => rule.id !== id));
  }, []);

  const reset = useCallback(() => {
    setRules([]);
    setMode("replace");
    setTolerance(30);
    setAllToOne("#CBE71F");
    setPreserveShading(false);
    setSaved(null);
    setZoom(1);
    // Tells the repaint effect to skip the debounce, so the preview reverts the
    // moment the button is pressed rather than half a second later.
    setUndoSignal((n) => n + 1);
  }, []);

  /**
   * Add every colour the image contains as its own rule, each mapped to itself.
   *
* Counts the colours on demand rather than at open, and waits for the list -
   * clicking this without the list would mean either a blank panel or adding
   * nothing at all while the button appeared to work.
   *
   * Rows start as no-ops rather than as a random palette, so nothing changes
   * until the user picks a target. Building 60 rows all pointed at red the moment
   * they clicked would destroy the image before they had decided anything.
   *
   * Takes the grouped list rather than the census. Sixty rows is already the
   * practical ceiling, and at the default tolerance sixty exact colours covered a
   * fraction of what sixty groups do - the rest of the file would have had no row
   * at all. The rows now line up with the swatches in the panel below.
   */
  const addAllExtracted = useCallback(async () => {
    const found = await loadColors();
    if (!found || found.colors.length === 0) return;

    const grouped = await invoke<ColorGroup[]>("recolor_group_colors", {
      colors: found.colors,
      tolerance,
    }).catch(() => null);

    const top = (grouped ?? found.colors).slice(0, 60);
    setRules((current) => {
      const known = new Set(current.map((rule) => rule.from));
      const additions: DraftRule[] = top
        .filter((entry) => !known.has(entry.color))
        .map((entry) => ({
          id: makeRuleId(),
          from: entry.color,
          to: entry.color,
          enabled: true,
        }));
      return [...current, ...additions];
    });
  }, [loadColors, tolerance]);

  const setToColor = useCallback(
    (hex: Hex) => {
      // One target for every row at once: "make the whole thing red" should not
      // mean clicking the picker five times.
      setRules((current) =>
        current.map((rule) => ({ ...rule, to: hex, enabled: true }))
      );
      setAllToOne(hex);
    },
    []
  );

  /** Ask the app where the file would go, then save there. */
  const save = useCallback(async () => {
    if (!source) return;
    setSaving(true);
    setError(null);
    try {
      setSaved(
        await invoke<RecolorSaved>("recolor_apply", {
          request: { input_path: source.path, format: format || null, params: payload },
        })
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }, [source, format, payload]);

  /** Save to a folder and filename the user chose. */
  const saveTo = useCallback(
    async (outputPath: string) => {
      if (!source) return;
      setSaving(true);
      setError(null);
      try {
        setSaved(
          await invoke<RecolorSaved>("recolor_apply", {
            request: {
              input_path: source.path,
              output_path: outputPath,
              format: format || null,
              params: payload,
            },
          })
        );
      } catch (e) {
        setError(String(e));
      } finally {
        setSaving(false);
      }
    },
    [source, format, payload]
  );

  const saveAs = useCallback(async () => {
    if (!source) return;
    const extension = (format || source.extension).replace(/^\./, "");
    const picked = await saveDialog({
      defaultPath: suggestName(source.file_name, extension),
      filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
    });
    if (typeof picked === "string") await saveTo(picked);
  }, [source, format, saveTo]);

  const targetFormat = format || source?.extension || "png";

  /**
   * The formats on offer for this file.
   *
   * SVG appears only when the source is an SVG, because that is the only case
   * where the save edits markup. Offering it for a PNG produced a save that died
   * with "stream did not contain valid UTF-8" - the tool trying to read PNG bytes
   * as text, which is exactly what the user should never have been invited to do.
   */
  const saveFormats = useMemo(
    () =>
      isSvgExtension(source?.extension)
        ? [...RASTER_FORMATS, SVG_FORMAT]
        : RASTER_FORMATS,
    [source?.extension]
  );

  /**
   * Whether the save will produce an SVG.
   *
   * Only true when the source is vector too, so this cannot drift away from what
   * `saveFormats` actually offers.
   */
  const savingVector = isSvgExtension(source?.extension) && isSvgExtension(targetFormat);

  // The alpha warning only applies to raster output. An SVG keeps transparency
  // natively, so warning about it there would be wrong.
  const losesTransparency =
    !savingVector &&
    source?.has_transparency &&
    OPAQUE_ONLY.has(targetFormat.toLowerCase());

  /*
   * A format left over from a previous file is not on offer for this one.
   *
   * The window can be reused for a different file - by choosing another, or by
   * the backend re-pointing it when the action is dispatched again - and "SVG"
   * would then linger in the dropdown pointing at a raster save. Falling back to
   * the source format is the safe reset, and it keeps the request and the menu
   * agreeing.
   */
  useEffect(() => {
    if (source && !saveFormats.some((f) => f.value === format)) {
      setFormat("");
    }
  }, [source, saveFormats, format]);

  return (
    <WindowFrame title="Recolor Image" bodyClassName="flex">
      <div className="flex flex-1 min-h-0">
        {/* ---- Preview -------------------------------------------------- */}
        <section className="flex-1 min-w-0 flex flex-col border-r border-line-subtle">
          <header className="h-[44px] shrink-0 flex items-center gap-3 px-4 border-b border-line-subtle">
            <div className="min-w-0">
              <div className="text-[13px] text-text truncate">{source?.file_name ?? "No image"}</div>
              <div className="text-[11px] text-text-muted">
                {source
                  ? `${source.width.toLocaleString()} × ${source.height.toLocaleString()} · ${formatBytes(source.file_size)}`
                  : "Open an image to begin"}
              </div>
            </div>

            <div className="ml-auto flex items-center gap-2">
              <button
                onClick={() => setShowOriginal((v) => !v)}
                className={`inline-flex items-center gap-1.5 px-2.5 py-1.5 text-[12px] rounded-md border transition-colors ${
                  showOriginal
                    ? "bg-accent/15 border-accent/40 text-accent"
                    : "bg-surface-control border-line-strong text-text-secondary hover:bg-surface-control-hover"
                }`}
                title="Hold to compare with the original"
              >
                {showOriginal ? <EyeOff size={13} /> : <Eye size={13} />}
                {showOriginal ? "Original" : "Recolored"}
              </button>
              <button
                onClick={reset}
                disabled={!source}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-[12px] rounded-md border border-line-strong bg-surface-control text-text-secondary hover:bg-surface-control-hover disabled:opacity-40 transition-colors"
                title="Undo every change"
              >
                <RotateCcw size={13} />
                Reset
              </button>
            </div>
          </header>

          <div ref={viewportRef} className="flex-1 min-h-0 relative">
            {/*
             * The scrolling layer is absolutely positioned inside a
             * content-free measuring box.
             *
             * It has to be: the panes have a pixel size that is computed from
             * the space available, so if the panes sat inside the element being
             * measured, their width would push that element wider, which would
             * grow the panes again. The window would only ever be able to get
             * bigger - resizing down did nothing, and the side panel ended up
             * pushed off the right edge. Measuring an empty box breaks the loop.
             */}
            <div className="absolute inset-0 overflow-auto bg-[repeating-conic-gradient(var(--color-surface-field)_0%_25%,var(--color-surface-window)_0%_50%)] bg-[length:16px_16px]">
              {loading ? (
                <EmptyState
                  icon={<Loader2 size={22} className="animate-spin" />}
                  title="Opening image"
                  detail="Reading pixels and counting colours"
                />
              ) : !source ? (
                <EmptyState
                  icon={<ImageOff size={22} />}
                  title={error ? "Could not open that image" : "No image open"}
                  detail={error ?? "Choose a PNG, JPEG, WebP, AVIF or SVG to recolour."}
                  action={
                    <button
                      onClick={chooseFile}
                      className="inline-flex items-center gap-2 px-3.5 py-2 text-[13px] font-medium rounded-lg bg-accent hover:bg-accent-hover text-accent-ink transition-colors"
                    >
                      <Plus size={14} />
                      Choose Image
                    </button>
                  }
                />
              ) : (
                <div className="min-h-full flex items-center justify-center p-4">
                  <div
                    className={
                      stacked
                        ? "flex flex-col items-center gap-3"
                        : "flex flex-row items-center gap-3"
                    }
                  >
                    <PreviewPane
                      label="Original"
                      canvasRef={originalCanvasRef}
                      display={displaySize}
                      onClick={pickFromImage.bind(null, "original")}
                      highlight={pickerOpen}
                      rules={activeRules}
                      tolerance={tolerance}
                      original={previewBuffer}
                      onLeave={() => clearHighlight([originalCanvasRef])}
                    />
                    <PreviewPane
                      label={showOriginal ? "Original" : "Recolored"}
                      canvasRef={previewCanvasRef}
                      display={displaySize}
                      onClick={pickFromImage.bind(null, "preview")}
                      highlight={pickerOpen}
                      rules={activeRules}
                      tolerance={tolerance}
                      original={previewBuffer}
                      onLeave={() => clearHighlight([previewCanvasRef])}
                    />
                  </div>
                </div>
              )}
            </div>
          </div>

          {source && (
            <footer className="h-[38px] shrink-0 flex items-center gap-3 px-4 border-t border-line-subtle text-[11px] text-text-muted">
              {/*
                The colour count only once it has been counted. Showing a
                placeholder here would either be a lie or would put the work back
                on the open path, which is what this change removed.
              */}
              {colors && (
                <>
                  <span>
                    {colors.total_unique.toLocaleString()} distinct colour
                    {colors.total_unique === 1 ? "" : "s"}
                  </span>
                  <span className="text-line-strong">|</span>
                </>
              )}
              <span>
                {mode === "all_to_one"
                  ? "Everything to one colour"
                  : `${activeRules.length} replacement${activeRules.length === 1 ? "" : "s"}`}
              </span>
              <div className="ml-auto flex items-center gap-1.5">
                <button
                  onClick={() => setZoom((z) => Math.max(0.25, Number((z - 0.25).toFixed(2))))}
                  className="w-6 h-6 rounded border border-line-strong bg-surface-control text-text-secondary hover:bg-surface-control-hover text-[13px] leading-none"
                  title="Zoom out"
                >
                  −
                </button>
                <span className="w-11 text-center tabular-nums">{Math.round(zoom * 100)}%</span>
                <button
                  onClick={() => setZoom((z) => Math.min(4, Number((z + 0.25).toFixed(2))))}
                  className="w-6 h-6 rounded border border-line-strong bg-surface-control text-text-secondary hover:bg-surface-control-hover text-[13px] leading-none"
                  title="Zoom in"
                >
                  +
                </button>
              </div>
            </footer>
          )}
        </section>

        {/* ---- Controls -------------------------------------------------
            Width is a clamp rather than a fixed value: a fixed 420px column
            leaves almost nothing for the preview once the window is dragged
            narrow, and the rule rows then have to wrap or clip. */}
        <aside className="w-[clamp(320px,36vw,430px)] shrink-0 flex flex-col bg-surface-window">
          <div className="shrink-0 p-4 border-b border-line-subtle">
            <SegmentedTabs value={mode} onChange={setMode} />
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto">
            {mode === "replace" ? (
              <>
                <ReplacePanel
                  rules={rules}
                  activeRules={activeRules}
                  tolerance={tolerance}
                  onTolerance={setTolerance}
                  onUpdate={updateRule}
                  onRemove={removeRule}
                  onAddManual={addManualRule}
                  onOpenPicker={() => setPickerOpen((v) => !v)}
                  onAddAll={addAllExtracted}
                  loadingColors={colorsLoading}
                  onSetToAll={setToColor}
                  livePreview={livePreview}
                  onLivePreview={setLivePreview}
                />
<ExtractedColors
                  extraction={colors}
                  groups={groups}
                  tolerance={tolerance}
                  complete={colorsComplete}
                  loading={colorsLoading}
                  error={colorsError}
                  limit={colorLimit}
                  onLimit={setColorLimit}
                  onRetry={() => {
                    colorsRequestRef.current = null;
                    void loadColors();
                  }}
                  onPick={(hex) => {
                    setRules((current) =>
                      current.some((rule) => rule.from === hex)
                        ? current.map((rule) =>
                            rule.from === hex ? { ...rule, enabled: true } : rule
                          )
                        : [
                            ...current,
                            { id: makeRuleId(), from: hex, to: hex, enabled: true },
                          ]
                    );
                    setPickerOpen(false);
                  }}
                  open={paletteOpen}
                  onToggle={() => setPaletteOpen((v) => !v)}
                />
              </>
            ) : (
              <AllToOnePanel
                color={allToOne}
                onColor={setAllToOne}
                preserveShading={preserveShading}
                onPreserveShading={setPreserveShading}
                livePreview={livePreview}
                onLivePreview={setLivePreview}
              />
            )}
          </div>

          {/* ---- Save ---------------------------------------------------- */}
          <div className="shrink-0 border-t border-line-subtle p-4 space-y-3">
            {pickerOpen && mode === "replace" && (
              <motion.div
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                className="flex items-start gap-2 p-2.5 rounded-lg bg-accent/10 border border-accent/25 text-[11px] text-text-secondary leading-relaxed"
              >
                <Pipette size={14} className="text-accent mt-px shrink-0" />
                <span className="flex-1">
                  Click a colour in either preview to add it as a replacement.
                </span>
                <button onClick={() => setPickerOpen(false)} className="text-text-muted hover:text-text">
                  <X size={12} />
                </button>
              </motion.div>
            )}

            {losesTransparency && (
              <div className="flex items-start gap-2 p-2.5 rounded-lg bg-caution-soft border border-caution/25 text-[11px] text-text-secondary leading-relaxed">
                <Info size={14} className="text-caution mt-px shrink-0" />
                <span>
                  {format.toUpperCase()} cannot store transparency. Transparent areas
                  will be filled with white.
                </span>
              </div>
            )}

            {error && source && (
              <div className="flex items-start gap-2 p-2.5 rounded-lg bg-negative-soft border border-negative/25 text-[11px] text-text-secondary leading-relaxed">
                <Info size={14} className="text-negative mt-px shrink-0" />
                <span className="flex-1 min-w-0 break-words">{error}</span>
                <button
                  onClick={() => setError(null)}
                  title="Dismiss"
                  className="shrink-0 text-text-muted hover:text-text"
                >
                  <X size={12} />
                </button>
              </div>
            )}

            {savingVector && (
              <div className="flex items-start gap-2 p-2.5 rounded-lg bg-accent/10 border border-accent/25 text-[11px] text-text-secondary leading-relaxed">
                <Sparkles size={14} className="text-accent mt-px shrink-0" />
                <span>
                  Saving as SVG edits the file&apos;s own markup. Paths, gradients and
                  sizing all survive, and it stays resolution independent.
                </span>
              </div>
            )}

            {isNoop(engineParams) && source && !saving && (
              <div className="flex items-start gap-2 p-2.5 rounded-lg bg-surface-raised border border-line-subtle text-[11px] text-text-muted leading-relaxed">
                <Info size={14} className="mt-px shrink-0" />
                <span>
                  Nothing is selected yet, so saving would copy the file unchanged.
                </span>
              </div>
            )}

            <div className="flex items-center gap-2">
              <label className="text-[11px] text-text-muted shrink-0">Save as</label>
              <select
                value={format}
                onChange={(e) => setFormat(e.target.value)}
                className="flex-1 min-w-0 bg-surface-field border border-line-strong rounded-md px-2 py-1.5 text-[12px] text-text"
              >
                {saveFormats.map((f) => (
                  <option key={f.value || "same"} value={f.value} className="bg-surface-control">
                    {f.value === "" && source?.extension
                      ? `Same as source (${source.extension.toUpperCase()})`
                      : f.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="flex items-center gap-2">
              <button
                onClick={save}
                disabled={!source || saving}
                className="flex-1 inline-flex items-center justify-center gap-2 px-4 py-2.5 text-[13px] font-semibold rounded-lg bg-accent hover:bg-accent-hover text-accent-ink disabled:opacity-40 transition-colors"
              >
                {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
                {saving ? "Saving" : "Save Recolored Copy"}
              </button>
              <button
                onClick={saveAs}
                disabled={!source || saving}
                title="Choose where to save"
                className="px-3 py-2.5 rounded-lg bg-surface-control hover:bg-surface-control-hover border border-line-strong text-text-secondary disabled:opacity-40 transition-colors"
              >
                Save as…
              </button>
            </div>

            {saved && (
              <motion.div
                initial={{ opacity: 0, y: -4 }}
                animate={{ opacity: 1, y: 0 }}
                className="flex items-center gap-2 p-2.5 rounded-lg bg-positive-soft border border-positive/25 text-[11px] text-text-secondary"
              >
                <Check size={14} className="text-positive shrink-0" />
                <span className="flex-1 min-w-0 truncate" title={saved.output_path}>
                  Saved {saved.file_name}
                  {saved.unchanged
                    ? " (unchanged)"
                    : saved.is_svg
                    ? ` (${saved.replaced_literals} colour${saved.replaced_literals === 1 ? "" : "s"} in the SVG)`
                    : ""}
                </span>
              </motion.div>
            )}
          </div>
        </aside>
      </div>
    </WindowFrame>
  );
}

/* ------------------------------------------------------------------ panes -- */

/**
 * One labelled canvas.
 *
 * The canvas is the only click target: when a colour picker is open, hovering
 * paints a ring on any pixel that the current rules would actually match, which
 * is the difference between "click the blue" and "click the right blue".
 */
function PreviewPane({
  label,
  canvasRef,
  display,
  onClick,
  highlight,
  rules,
  tolerance,
  original,
  onLeave,
}: {
  label: string;
  canvasRef: React.RefObject<HTMLCanvasElement | null>;
  display: { width: number; height: number };
  onClick: (event: React.MouseEvent<HTMLCanvasElement>) => void;
  /** Whether the eyedropper is armed on this pane. */
  highlight: boolean;
  rules: RecolorRule[];
  tolerance: number;
  original: ImageData | null;
  onLeave: () => void;
}) {
  return (
    <figure className="relative shrink-0">
      <canvas
        ref={canvasRef}
        onClick={onClick}
        onMouseMove={(e) =>
          highlight && hoverHighlight(e, canvasRef, rules, tolerance, original)
        }
        onMouseLeave={onLeave}
        // Sized through CSS, never through the width/height attributes: React
        // rewrites those on every render, and writing to a canvas attribute
        // resets its backing store, which would wipe the pixels on every zoom.
        // The backing store is set once, when the image is decoded.
        style={{
          width: display.width ? `${display.width}px` : undefined,
          height: display.height ? `${display.height}px` : undefined,
        }}
        className={`block rounded-lg border border-line shadow-lg ${
          highlight ? "cursor-crosshair" : ""
        }`}
      />
      <figcaption className="absolute top-2 left-2 px-2 py-0.5 rounded bg-black/55 text-[10px] font-medium tracking-wide text-white/90 pointer-events-none">
        {label}
      </figcaption>
    </figure>
  );
}

/**
 * Outline the pixel under the cursor when it matches a rule.
 *
 * Uses a CSS outline on the canvas rather than a second overlay canvas, so there
 * is one less thing to keep in sync with the buffer.
 */
function hoverHighlight(
  event: React.MouseEvent<HTMLCanvasElement>,
  canvasRef: React.RefObject<HTMLCanvasElement | null>,
  rules: RecolorRule[],
  tolerance: number,
  original: ImageData | null
) {
  const canvas = canvasRef.current;
  if (!canvas || !original || rules.length === 0) return;

  const bounds = canvas.getBoundingClientRect();
  const x = Math.floor((event.clientX - bounds.left) * (canvas.width / bounds.width));
  const y = Math.floor((event.clientY - bounds.top) * (canvas.height / bounds.height));

  // Read from the source buffer, not the canvas. The canvas holds the
  // *recoloured* result, so after a rule is applied its pixels no longer match
  // the rule they came from, and the highlight would light up the wrong places.
  if (!original || x < 0 || y < 0 || x >= original.width || y >= original.height) {
    canvas.style.outline = "";
    return;
  }

  const offset = (y * original.width + x) * 4;
  if (original.data[offset + 3] === 0) {
    canvas.style.outline = "";
    return;
  }

  const color = {
    r: original.data[offset],
    g: original.data[offset + 1],
    b: original.data[offset + 2],
  };
  // Uses the shared match test, so the ring appears exactly where the recolour
  // would actually land rather than on a second, slightly different rule.
  const hit = rules.some((rule) => {
    const from = parseHex(rule.from);
    return from ? matchesRule(color, from, tolerance) : false;
  });

  canvas.style.outline = hit ? "2px solid var(--color-accent)" : "";
  canvas.style.outlineOffset = "-2px";
}

/**
 * Drop the hover ring from every pane.
 *
 * Driven by state rather than a `querySelectorAll` sweep, so the two canvases
 * are cleared by one render and a pane that unmounts cannot leave a ring on
 * something else.
 */
function clearHighlight(
  refs: React.RefObject<HTMLCanvasElement | null>[]
) {
  for (const ref of refs) {
    if (ref.current) ref.current.style.outline = "";
  }
}

function EmptyState({
  icon,
  title,
  detail,
  action,
}: {
  icon: React.ReactNode;
  title: string;
  detail: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="h-full w-full flex flex-col items-center justify-center gap-3 text-center px-8">
      <div className="w-12 h-12 rounded-xl bg-surface-raised border border-line-subtle flex items-center justify-center text-text-muted">
        {icon}
      </div>
      <div>
        <div className="text-[14px] text-text">{title}</div>
        <div className="text-[12px] text-text-muted mt-1 max-w-[280px] leading-relaxed">
          {detail}
        </div>
      </div>
      {action}
    </div>
  );
}

/* ----------------------------------------------------------------- panels -- */

function SegmentedTabs({
  value,
  onChange,
}: {
  value: RecolorMode;
  onChange: (mode: RecolorMode) => void;
}) {
  const tabs: { value: RecolorMode; label: string; icon: React.ReactNode }[] = [
    { value: "replace", label: "Replace Colors", icon: <Palette size={14} /> },
    { value: "all_to_one", label: "All to One", icon: <Wand2 size={14} /> },
  ];

  return (
    <div role="tablist" className="flex gap-1 p-1 rounded-lg bg-surface-control border border-line-subtle">
      {tabs.map((tab) => {
        const active = tab.value === value;
        return (
          <button
            key={tab.value}
            role="tab"
            aria-selected={active}
            onClick={() => onChange(tab.value)}
            className={`flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-2 text-[12px] font-medium rounded-[7px] transition-colors ${
              active
                ? "bg-accent text-accent-ink"
                : "text-text-secondary hover:bg-surface-control-hover"
            }`}
          >
            {tab.icon}
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}

function ReplacePanel({
  rules,
  activeRules,
  tolerance,
  onTolerance,
  onUpdate,
  onRemove,
  onAddManual,
  onOpenPicker,
  onAddAll,
  loadingColors = false,
  onSetToAll,
  livePreview,
  onLivePreview,
}: {
  rules: DraftRule[];
  activeRules: DraftRule[];
  tolerance: number;
  onTolerance: (t: number) => void;
  onUpdate: (id: string, patch: Partial<DraftRule>) => void;
  onRemove: (id: string) => void;
  onAddManual: () => void;
  onOpenPicker: () => void;
  /** Optional: the caller omits it when it has no extracted list to offer. */
  onAddAll?: () => Promise<void> | void;
  /** True while the colour count is being fetched, so the button can say so. */
  loadingColors?: boolean;
  onSetToAll: (hex: Hex) => void;
  livePreview: boolean;
  onLivePreview: (v: boolean) => void;
}) {
  return (
    <div className="p-4 space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-[12px] font-semibold uppercase tracking-wider text-text-muted">
          Color replacements
        </h3>
        <span className="text-[11px] text-text-faint">
          {activeRules.length} of {rules.length} active
        </span>
      </div>

      <div className="flex items-center gap-2">
        <button
          onClick={onOpenPicker}
          className={`flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-2 text-[12px] font-medium rounded-lg border transition-colors ${
            rules.length > 0
              ? "bg-surface-control hover:bg-surface-control-hover border-line-strong text-text"
              : "bg-accent text-accent-ink border-transparent hover:bg-accent-hover"
          }`}
        >
          <Pipette size={13} />
          Pick from Image
        </button>
        <button
          onClick={onAddManual}
          title="Add a replacement by typing hex values"
          className="px-2.5 py-2 rounded-lg bg-surface-control hover:bg-surface-control-hover border border-line-strong text-text-secondary transition-colors"
        >
          <Plus size={13} />
        </button>
      </div>

      {rules.length === 0 ? (
        <div className="rounded-lg border border-dashed border-line-strong bg-surface-raised/40 px-4 py-8 text-center">
          <Layers size={18} className="mx-auto text-text-faint mb-2" />
          <p className="text-[12px] text-text-secondary">
            No colors selected yet
          </p>
          <p className="text-[11px] text-text-muted mt-1 leading-relaxed">
            Click <span className="text-accent">Pick from Image</span>, then click any
            colour in the preview to replace it.
          </p>
        </div>
      ) : (
          <ul className="space-y-1.5">
          {rules.map((rule) => (
            <RuleRow
              key={rule.id}
              rule={rule}
              onUpdate={(patch) => onUpdate(rule.id, patch)}
              onRemove={() => onRemove(rule.id)}
            />
          ))}
        </ul>
      )}

      {rules.length > 1 && (
        <button
          onClick={() => onSetToAll(activeRules[0]?.to ?? "#FF0000")}
          className="w-full inline-flex items-center justify-center gap-1.5 px-3 py-2 text-[12px] rounded-lg bg-surface-control hover:bg-surface-control-hover border border-line-strong text-text-secondary transition-colors"
        >
          <Copy size={13} />
          Send all to the first target
        </button>
      )}

      {onAddAll && (
        <button
          onClick={() => void onAddAll()}
          disabled={loadingColors}
          title="Add every colour found in the image as its own row"
          className="w-full inline-flex items-center justify-center gap-1.5 px-3 py-2 text-[12px] rounded-lg bg-surface-control hover:bg-surface-control-hover border border-line-strong text-text-secondary disabled:opacity-50 transition-colors"
        >
          {loadingColors ? (
            <Loader2 size={13} className="animate-spin" />
          ) : (
            <Sparkles size={13} />
          )}
          {loadingColors ? "Finding colours…" : "Auto add all colours"}
        </button>
      )}

      {/* ---- Tolerance ------------------------------------------------- */}
      <div className="pt-3 mt-1 border-t border-line-subtle">
        <div className="flex items-center justify-between mb-2">
          <label htmlFor="tolerance" className="text-[12px] text-text-secondary flex items-center gap-1.5">
            Match tolerance
            <span title="How far a colour may differ from the one you picked and still be replaced. 0 replaces only that exact colour.">
              <Info size={12} className="text-text-faint" />
            </span>
          </label>
          <input
            type="number"
            min={0}
            max={100}
            value={tolerance}
            onChange={(e) => {
              const next = Number(e.target.value);
              if (!Number.isNaN(next)) onTolerance(Math.max(0, Math.min(100, next)));
            }}
            className="w-14 text-right bg-surface-field border border-line-strong rounded-md px-2 py-1 text-[12px] tabular-nums text-text"
          />
        </div>
        <input
          id="tolerance"
          type="range"
          min={0}
          max={100}
          value={tolerance}
          onChange={(e) => onTolerance(Number(e.target.value))}
          className="w-full accent-[var(--color-accent)]"
        />
        <div className="flex justify-between text-[10px] text-text-faint mt-1">
          <span>Exact match</span>
          <span>Loose</span>
        </div>
        <p className="text-[10px] text-text-faint mt-1.5 leading-relaxed">
          The preview catches up half a second after you stop changing it.
        </p>
      </div>

      <div className="flex items-center justify-between pt-1">
        <span className="text-[12px] text-text-secondary">Preview changes as I edit</span>
        <ToggleSwitch checked={livePreview} onChange={onLivePreview} />
      </div>
    </div>
  );
}

/**
 * Every distinct colour in the file, as a swatch grid.
 *
 * This is the literal answer to "what colours are actually in this image" -
 * exact counts, no quantisation, so two pixels differing by one unit really do
 * appear as two entries. Clicking a swatch adds it as a replacement, which is
 * the same action as clicking it in the preview, just faster when you already
 * know the colour you want.
 *
 * The count is not fetched when the window opens. It is a full pass over every
 * pixel, and it happens the first time this panel is expanded instead.
 */
/** One swatch, whichever list it came from. */
type Swatch = {
  color: Hex;
  count: number;
  coverage: number;
  /** Distinct colours folded into this swatch at the current tolerance. */
  merged: number;
};

function ExtractedColors({
  extraction,
  groups,
  tolerance,
  complete,
  loading,
  error,
  limit,
  onLimit,
  onPick,
  onRetry,
  open,
  onToggle,
}: {
  /** Null until the count has been asked for. */
  extraction: ColorExtraction | null;
  /** The census folded into swatches; null while regrouping or on failure. */
  groups: ColorGroup[] | null;
  tolerance: number;
  complete: boolean;
  loading: boolean;
  error: string | null;
  limit: number;
  onLimit: (n: number) => void;
  onPick: (hex: Hex) => void;
  onRetry: () => void;
  open: boolean;
  onToggle: () => void;
}) {
  /**
   * What the swatches show.
   *
   * The groups when they are ready, because that is what a rule repaints. The
   * census is the fallback, so the panel never blanks out mid-slider.
   */
  const entries = useMemo<Swatch[]>(() => {
    const source: (ColorGroup | ExtractedColor)[] =
      groups ?? extraction?.colors ?? [];
    return source.map((entry) => ({
      color: entry.color,
      count: entry.count,
      coverage: entry.coverage,
      merged: "merged" in entry ? entry.merged : 1,
    }));
  }, [groups, extraction]);

  const grouped = groups !== null && groups.length !== (extraction?.colors.length ?? -1);

  const [query, setQuery] = useState("");
  const needle = query.trim().replace(/^#/, "").toUpperCase();

  const filtered = needle
    ? entries.filter((entry) =>
        entry.color.toUpperCase().replace(/^#/, "").includes(needle)
      )
    : entries;

  const total = extraction?.total_unique ?? 0;
  const shown = filtered.slice(0, limit);
  const remaining = filtered.length - shown.length;

  const percent = (value: number) =>
    `${(value * 100).toFixed(value < 0.01 ? 2 : 1)}%`;

  return (
    <section className="border-t border-line-subtle">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-4 py-3 text-left hover:bg-overlay transition-colors"
      >
        <List size={13} className="text-text-muted shrink-0" />
        <span className="text-[12px] font-semibold uppercase tracking-wider text-text-muted">
          Extracted colors
        </span>
        {/*
          No count until there is one to show. Inventing a placeholder would be a
          lie, and computing it here would put the work back on the open path,
          which is the whole thing this panel avoids.
        */}
        {loading ? (
          <Loader2 size={12} className="text-text-faint animate-spin" />
        ) : (
          entries.length > 0 && (
            <span className="text-[11px] text-text-faint">
              {grouped
                ? `${entries.length.toLocaleString()} at ${tolerance}`
                : total.toLocaleString()}
              {!complete && extraction && extraction.colors.length < total ? "+" : ""}
            </span>
          )
        )}
        <ChevronDown
          size={14}
          className={`ml-auto text-text-faint transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open && (
        <div className="px-4 pb-4">
          {loading && !extraction ? (
            <div className="flex items-center gap-2 py-6 justify-center text-[11px] text-text-muted">
              <Loader2 size={13} className="animate-spin" />
              Counting colours…
            </div>
          ) : error && !extraction ? (
            <div className="rounded-lg bg-negative-soft border border-negative/25 p-3">
              <div className="text-[11px] text-text-secondary leading-relaxed break-words">
                {error}
              </div>
              <button
                onClick={onRetry}
                className="mt-2 px-2.5 py-1 text-[11px] rounded bg-surface-control hover:bg-surface-control-hover border border-line-strong text-text-secondary transition-colors"
              >
                Try again
              </button>
            </div>
          ) : !extraction ? (
            <div className="text-[11px] text-text-faint py-6 text-center">
              Open this panel to count the colours in the image.
            </div>
          ) : (
<>
          <p className="text-[11px] text-text-muted leading-relaxed mb-3">
            {grouped ? (
              <>
                Each swatch is everything one replacement repaints at tolerance{" "}
                {tolerance}. This file holds {total.toLocaleString()} distinct
                colour{total === 1 ? "" : "s"}; the rest are shades the same rule
                catches. Lower the tolerance to separate them.
              </>
            ) : (
              <>
                Every distinct colour found in this file, counted exactly. Click one
                to add it as a replacement.
              </>
            )}
          </p>

          {total === 0 ? (
            <div className="text-[11px] text-text-faint py-4 text-center">
              This image is fully transparent, so it has no colours to replace.
            </div>
          ) : (
            <>
              {/*
                With hundreds of shades a list needs to be findable. At tolerance
                0 this is the only way to reach a colour that is not in the first
                page.
              */}
              {entries.length > 24 && (
                <div className="relative mb-3">
                  <Search
                    size={12}
                    className="absolute left-2.5 top-1/2 -translate-y-1/2 text-text-faint pointer-events-none"
                  />
                  <input
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder="Find a color, e.g. 2EA"
                    aria-label="Filter colours"
                    className="w-full pl-7 pr-2.5 py-1.5 text-[12px] rounded-md bg-surface-control border border-line-strong text-text-primary placeholder:text-text-faint focus:border-accent focus:outline-none transition-colors"
                  />
                </div>
              )}

              {needle && filtered.length === 0 && (
                <div className="text-[11px] text-text-faint py-4 text-center">
                  Nothing matches {query}.
                </div>
              )}

              {filtered.length > 0 && (
                <div className="grid grid-cols-[repeat(auto-fill,minmax(34px,1fr))] gap-1.5">
                  {shown.map((entry) => {
                    const rgb = parseHex(entry.color);
                    if (!rgb) return null;
                    const merged = entry.merged;
                    return (
                      <button
                        key={entry.color}
                        onClick={() => onPick(entry.color)}
                        title={
                          merged > 1
                            ? `${entry.color} · ${merged.toLocaleString()} colours · ` +
                              `${entry.count.toLocaleString()} px (${percent(entry.coverage)})`
                            : `${entry.color} · ${entry.count.toLocaleString()} px ` +
                              `(${percent(entry.coverage)})`
                        }
                        aria-label={`Replace ${entry.color}${
                          merged > 1 ? ` and ${merged - 1} nearby colours` : ""
                        }`}
                        className="relative aspect-square rounded-md ring-1 ring-inset ring-black/30 hover:ring-2 hover:ring-accent transition-all"
                        style={{ background: toCssRgb(rgb) }}
                      >
                        {/*
                          The merged count is drawn because it is the thing that
                          decides whether clicking a swatch is a one-pixel change
                          or a rewrite of a third of the image. Everything else is
                          in the tooltip, where there is room for it.
                        */}
                        {merged > 1 && (
                          <span className="absolute bottom-0 right-0 px-1 text-[9px] font-semibold leading-[13px] rounded-tl bg-black/55 text-white/90 tabular-nums">
                            {merged}
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
              )}

              {remaining > 0 && (
                <button
                  onClick={() => onLimit(limit + COLORS_PAGE)}
                  className="mt-3 w-full px-3 py-2 text-[12px] rounded-lg bg-surface-control hover:bg-surface-control-hover border border-line-strong text-text-secondary transition-colors"
                >
                  Show {Math.min(remaining, COLORS_PAGE).toLocaleString()} more
                  {needle ? " matching" : complete && extraction && extraction.colors.length === remaining
                    ? ""
                    : ` · ${remaining.toLocaleString()} loaded`}
                </button>
              )}

              {!complete && extraction && (
                <p className="text-[10px] text-text-faint mt-2 leading-relaxed">
                  Showing the {extraction.colors.length.toLocaleString()} most-used of{" "}
                  {total.toLocaleString()}. Rare colours further down the list are
                  not loaded.
                </p>
              )}
            </>
          )}
          </>
          )}
        </div>
      )}
    </section>
  );
}

function RuleRow({
  rule,
  onUpdate,
  onRemove,
}: {
  rule: DraftRule;
  onUpdate: (patch: Partial<DraftRule>) => void;
  onRemove: () => void;
}) {
  // A row whose target still equals its source is a no-op. Saying so inline
  // stops the user wondering why the preview did not move.
  const isNoopRow = rule.from.toUpperCase() === rule.to.toUpperCase();

  return (
    <li className="flex items-center gap-2 p-2 rounded-lg bg-surface-raised border border-line-subtle">
      <button
        role="checkbox"
        aria-checked={rule.enabled}
        onClick={() => onUpdate({ enabled: !rule.enabled })}
        title={rule.enabled ? "Turn this replacement off" : "Turn this replacement on"}
        className={`w-4 h-4 shrink-0 rounded border flex items-center justify-center transition-colors ${
          rule.enabled
            ? "bg-accent border-accent text-accent-ink"
            : "border-line-strong bg-surface-control"
        }`}
      >
        {rule.enabled && <Check size={11} strokeWidth={3} />}
      </button>

      <ColorField value={rule.from} onChange={(hex) => onUpdate({ from: hex })} />

      <ArrowRight size={12} className="shrink-0 text-text-faint" />

      <ColorField value={rule.to} onChange={(hex) => onUpdate({ to: hex })} />

      {isNoopRow && (
        <span
          className="shrink-0 text-[10px] text-text-faint"
          title="Pick a different colour for this row to see a change"
        >
          same
        </span>
      )}

      <button
        onClick={onRemove}
        title="Remove this replacement"
        className="shrink-0 p-1 rounded text-text-faint hover:text-negative hover:bg-negative-soft transition-colors"
      >
        <Trash2 size={13} />
      </button>
    </li>
  );
}

/**
 * A swatch paired with a hex field.
 *
 * The native colour input is used for picking because it is the only control
 * that gives a real OS colour wheel; the text field stays editable because
 * pasting a hex value is faster than dragging for anyone who knows the colour.
 * A value that will not parse turns the border red and keeps the last good
 * colour on the swatch, so a half-typed hex never blanks the preview.
 */
function ColorField({
  value,
  onChange,
}: {
  value: Hex;
  onChange: (hex: Hex) => void;
}) {
  const [draft, setDraft] = useState(value);
  const [invalid, setInvalid] = useState(false);

  useEffect(() => {
    setDraft(value);
    setInvalid(false);
  }, [value]);

  const rgb = parseHex(value);

  return (
    <div
      className={`flex-1 min-w-0 flex items-center gap-1.5 rounded-md border bg-surface-field px-1.5 py-1 ${
        invalid ? "border-negative" : "border-line-subtle focus-within:border-accent/60"
      }`}
    >
      <label className="relative w-4 h-4 shrink-0 rounded overflow-hidden ring-1 ring-inset ring-black/30">
        <span
          className="absolute inset-0"
          style={{
            background: rgb
              ? toCssRgb(rgb)
              : "repeating-conic-gradient(#666 0% 25%, #888 0% 50%) 50% / 8px 8px",
          }}
        />
        <input
          type="color"
          value={value}
          onChange={(e) => onChange(e.target.value.toUpperCase())}
          className="absolute inset-0 opacity-0 w-full h-full"
          title="Open the colour picker"
        />
      </label>
      <input
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
          const parsed = normalizeHex(e.target.value);
          if (parsed) {
            setInvalid(false);
            onChange(parsed);
          } else {
            setInvalid(true);
          }
        }}
        onBlur={() => {
          setDraft(value);
          setInvalid(false);
        }}
        spellCheck={false}
        className="w-full min-w-0 bg-transparent text-[11px] font-mono uppercase text-text outline-none"
        aria-label="Hex colour"
      />
    </div>
  );
}

function AllToOnePanel({
  color,
  onColor,
  preserveShading,
  onPreserveShading,
  livePreview,
  onLivePreview,
}: {
  color: Hex;
  onColor: (hex: Hex) => void;
  preserveShading: boolean;
  onPreserveShading: (v: boolean) => void;
  livePreview: boolean;
  onLivePreview: (v: boolean) => void;
}) {
  const rgb = parseHex(color);

  return (
    <div className="p-4 space-y-4">
      <div>
        <h3 className="text-[12px] font-semibold uppercase tracking-wider text-text-muted mb-3">
          One color for everything
        </h3>
        <p className="text-[11px] text-text-muted leading-relaxed mb-3">
          Every visible pixel becomes this colour. Transparent areas are left
          alone.
        </p>

        <div className="flex items-center gap-3 p-3 rounded-lg bg-surface-raised border border-line-subtle">
          <label className="relative w-11 h-11 shrink-0 rounded-lg overflow-hidden ring-1 ring-inset ring-black/40">
            <span
              className="absolute inset-0"
              style={{ background: rgb ? toCssRgb(rgb) : "transparent" }}
            />
            <input
              type="color"
              value={color}
              onChange={(e) => onColor(e.target.value.toUpperCase())}
              className="absolute inset-0 opacity-0 w-full h-full"
              title="Choose the colour"
            />
          </label>
          <div className="min-w-0">
            <input
              value={color}
              onChange={(e) => {
                const parsed = normalizeHex(e.target.value);
                if (parsed) onColor(parsed);
              }}
              spellCheck={false}
              className="w-full bg-surface-field border border-line-strong rounded-md px-2 py-1.5 text-[12px] font-mono uppercase text-text outline-none focus:border-accent/60"
              aria-label="Hex colour"
            />
            {!rgb && (
              <div className="text-[11px] text-negative mt-1">Not a valid colour</div>
            )}
          </div>
        </div>
      </div>

      <div className="flex items-start justify-between gap-4 p-3 rounded-lg bg-surface-raised border border-line-subtle">
        <div className="min-w-0">
          <div className="text-[12px] text-text">Preserve shading</div>
          <div className="text-[11px] text-text-muted mt-0.5 leading-relaxed">
            Keeps each pixel's lightness so the image keeps its detail. Off by
            default, which gives a completely flat silhouette.
          </div>
        </div>
        <ToggleSwitch checked={preserveShading} onChange={onPreserveShading} />
      </div>

      <div className="flex items-center justify-between">
        <span className="text-[12px] text-text-secondary">Preview changes as I edit</span>
        <ToggleSwitch checked={livePreview} onChange={onLivePreview} />
      </div>
      <p className="text-[10px] text-text-faint leading-relaxed">
        The preview catches up half a second after you stop changing it.
      </p>
    </div>
  );
}

/* ---------------------------------------------------------------- helpers -- */

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** `Bird.png` + `png` → `Bird.recolored.png`, for the save-as default. */
function suggestName(fileName: string, extension: string): string {
  const dot = fileName.lastIndexOf(".");
  const stem = dot > 0 ? fileName.slice(0, dot) : fileName;
  return `${stem}.recolored.${extension.replace(/^\./, "")}`;
}
