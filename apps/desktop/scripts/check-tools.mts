/**
 * Confirms every tool the Rust registry advertises actually reaches the wheel.
 *
 * The original bug this guards against: `tool.recolor` was registered in
 * `wheel-core` and the tool window worked perfectly when opened directly, but
 * the wheel drew its tools from a hardcoded frontend list that only ever had
 * `tool.trim`. Nothing failed, no test went red, and the tool was simply not
 * there. This runs the wheel's own builder against the manifests the backend
 * really sends, so the two can never drift apart silently again.
 *
 * Run: `pnpm --filter desktop run test:tools`
 */
// Vite resolves the extensionless specifiers the app itself uses.
import { getPetalsForPage, filterActions } from "../src/windows/overlay/RadialWheel";
import type { ActionManifest } from "../src/store/wheelStore";

/**
 * The manifests `get_actions` returns.
 *
 * Hand-copied from `wheel-core/src/action.rs`, which is the real source. Kept in
 * sync by `tool.recolor_is_registered_as_a_single_image_tool_window` in the Rust
 * suite, which pins the fields this file depends on. If a tool is added in Rust
 * and not here, `every_registered_tool_reaches_the_tools_page` fails - which is
 * the point.
 */
/** `TRANSPARENT_IMAGE_EXTS` in RadialWheel, which the tools accept. */
const TRANSPARENT_EXTS = [
  "png", "webp", "avif", "tiff", "tif", "gif", "ico", "svg", "heic",
];

const MANIFESTS: ActionManifest[] = [
  {
    id: "tool.trim",
    title: "TRIM",
    icon: "trim",
    category: "tools",
    accepts: { extensions: TRANSPARENT_EXTS, multi: true },
    kind: "instant",
    enabled: true,
    order: 0,
  },
  {
    id: "tool.recolor",
    title: "Recolor Image",
    icon: "recolor",
    category: "tools",
    // Matches `transparent_image_exts` in wheel-core: no JPEG, because those
    // have no alpha to preserve. Recolor handles transparency correctly and
    // warns before flattening it, which is why JPEG is excluded rather than
    // silently allowed.
    accepts: { extensions: TRANSPARENT_EXTS, multi: false },
    kind: "window",
    window: { width: 1180, height: 780, resizable: true, mica: true },
    enabled: true,
    order: 1,
  },
];

let failures = 0;
let CHECK_COUNT = 0;

function check(name: string, condition: boolean, detail = "") {
  CHECK_COUNT++;
  if (condition) {
    console.log(`✓ ${name}`);
  } else {
    failures++;
    console.error(`✗ ${name}${detail ? ` — ${detail}` : ""}`);
  }
}

/**
 * Ids the wheel draws on its Tools page.
 *
 * `filterActions` always applies the context filter, so turning it off has to be
 * tested through `getPetalsForPage`, which takes the flag directly.
 */
function drawnToolIds(
  extensions: string[],
  slotCount: number,
  contextFilter = true
): string[] {
  const source = contextFilter
    ? filterActions(MANIFESTS, "tools", extensions, true, slotCount)
    : getPetalsForPage("tools", extensions, slotCount, false, MANIFESTS);
  return source.map((a) => a.id);
}

const registeredTools = MANIFESTS.filter((m) => m.category === "tools").map((m) => m.id);

check(
  "every registered tool reaches the tools page",
  registeredTools.every((id) => drawnToolIds(["png"], 8).includes(id)),
  `registered: ${registeredTools.join(", ")}; drawn: ${drawnToolIds(["png"], 8).join(", ")}`
);

check(
  "no extra tool is drawn that the registry does not advertise",
  drawnToolIds(["png"], 8).every((id) => registeredTools.includes(id)),
  `drawn: ${drawnToolIds(["png"], 8).join(", ")}`
);

check(
  "a low slot_count does not hide tools",
  drawnToolIds(["png"], 1).length === registeredTools.length,
  `slot_count 1 drew ${drawnToolIds(["png"], 1).join(", ")}`
);

check(
  "the tools page still renders before the registry loads",
  getPetalsForPage("tools", ["png"], 8, true).length > 0,
  "no petals while get_actions is in flight"
);

check(
  "recolor is offered for the formats its manifest accepts",
  ["png", "webp", "avif", "gif", "svg"].every((ext) =>
    drawnToolIds([ext], 8).includes("tool.recolor")
  )
);

check(
  "an opaque format with no alpha is not offered",
  // JPEG is excluded on purpose: it cannot store transparency, and recoloring
  // one would silently flatten whatever the user had made transparent.
  !drawnToolIds(["jpg"], 8).includes("tool.recolor"),
  `drew ${drawnToolIds(["jpg"], 8).join(", ")}`
);

check(
  "an unrelated file type offers no tools",
  drawnToolIds(["psd"], 8).length === 0,
  `drew ${drawnToolIds(["psd"], 8).join(", ")}`
);

check(
  "turning the context filter off shows every tool",
  drawnToolIds(["psd"], 8, false).length === registeredTools.length,
  `drew ${drawnToolIds(["psd"], 8, false).join(", ")}`
);

// Geometry must be recomputed for however many tools there are, or the wedges
// overlap and only the first is ever hit-testable.
const twoTools = getPetalsForPage("tools", ["png"], 8, true, MANIFESTS);
const oneTool = getPetalsForPage("tools", ["png"], 8, true, MANIFESTS.slice(0, 1));
check(
  "wedges are laid out per tool, not all stacked at one angle",
  twoTools.length === 2 &&
    twoTools[0].d !== twoTools[1].d &&
    twoTools[0].labelY !== twoTools[1].labelY,
  `two tools at labels ${twoTools.map((p) => p.labelY).join(", ")}`
);

check(
  "a single tool still fills the whole ring",
  oneTool.length === 1
);

// Manifests are the source of truth for the list, so a tool present in the
// registry but absent from the frontend catalog must still appear.
const recolorPetal = twoTools.find((p) => p.id === "tool.recolor");
check(
  "a tool with no frontend catalog entry still gets a label",
  !!recolorPetal && recolorPetal.title.length > 0 && recolorPetal.subtitle.length > 0,
  `title=${recolorPetal?.title} subtitle=${recolorPetal?.subtitle}`
);

check(
  "a catalog entry the registry does not advertise is not drawn",
  !drawnToolIds(["png"], 8).includes("tool.not-registered")
);

// The specific regression: recolor was registered in Rust, its window worked, and
// the wheel still showed only TRIM. Assert it by name so the failure says what
// actually went wrong rather than reporting a count mismatch.
check(
  "tool.recolor appears on the Tools page",
  drawnToolIds(["png"], 8).includes("tool.recolor"),
  `the wheel drew: ${drawnToolIds(["png"], 8).join(", ") || "nothing"}`
);

check(
  "tool.trim still appears on the Tools page",
  drawnToolIds(["png"], 8).includes("tool.trim")
);

if (failures > 0) {
  console.error(`\n${failures} check(s) failed.`);
  process.exit(1);
}

const passed = CHECK_COUNT;
console.log(`\nAll ${passed} checks passed.`);
