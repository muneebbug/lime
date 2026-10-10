// Tools gating: the wheel must offer exactly the tools the backend accepts.
//
//   pnpm run test:tools
//
// This compiles the wheel module and exercises it, because the failure it guards
// against cannot be seen from either side alone: the wheel drew its petals from
// a fallback catalog while the overlay decided hover from the registry, so a JPEG
// painted two tools and hovered neither.

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const out = join(root, ".tools-check");

/** The registry, mirrored from crates/wheel-core/src/action.rs. */
const manifest = {
  "tool.trim": { exts: ["png", "webp", "avif", "tiff", "tif", "gif", "ico", "heic"] },
  "tool.recolor": {
    exts: [
      "png", "webp", "avif", "tiff", "tif", "gif", "ico", "heic",
      "jpg", "jpeg", "bmp",
      "svg",
    ],
  },
};

function loadWheel() {
  // pnpm hoists to the workspace root, which is two levels above this package.
  const tsc = join(
    root,
    "..",
    "..",
    "node_modules",
    ".pnpm",
    "typescript@6.0.3",
    "node_modules",
    "typescript",
    "bin",
    "tsc"
  );
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  try {
    execFileSync(
      process.execPath,
      [
        tsc,
        "--ignoreConfig",
        "src/windows/overlay/RadialWheel.tsx",
        "--outDir",
        out,
        "--module",
        "commonjs",
        "--target",
        "es2022",
        "--jsx",
        "react-jsx",
        "--moduleResolution",
        "node",
        // node10 resolution is deprecated and tsc exits non-zero over it even
        // though it still emits, so the warning has to be silenced rather than
        // treated as a failure.
        "--ignoreDeprecations",
        "6.0",
        "--skipLibCheck",
        "--esModuleInterop",
      ],
      { cwd: root, stdio: "pipe" }
    );
  } catch (error) {
    // Only care whether it emitted; a real failure leaves no output.
    if (!existsSync(join(out, "windows", "overlay", "RadialWheel.js"))) throw error;
  }
  // `package.json` has "type": "module", so the emitted CommonJS needs the
  // extension changed before require() will touch it.
  const compiled = join(out, "windows", "overlay", "RadialWheel.js");
  const asCjs = compiled.replace(/\.js$/, ".cjs");
  copyFileSync(compiled, asCjs);
  return createRequire(import.meta.url)(asCjs);
}

const failures = [];
function check(label, actual, expected) {
  const a = JSON.stringify(actual);
  const e = JSON.stringify(expected);
  if (a !== e) failures.push(`${label}\n      expected ${e}\n      actual   ${a}`);
}

const wheel = loadWheel();
const manifests = Object.entries(manifest).map(([id, def]) => ({
  id,
  title: id,
  icon: "trim",
  category: "tools",
  enabled: true,
  accepts: { extensions: def.exts, multi: true },
}));

// What the user asked for, pinned per format.
const expectedTools = {
  jpg: ["tool.recolor"],
  jpeg: ["tool.recolor"],
  bmp: ["tool.recolor"],
  // Trim rasterises an SVG and cannot write it back as a vector, so it failed
  // on save. Recolor edits the markup and works.
  svg: ["tool.recolor"],
  png: ["tool.trim", "tool.recolor"],
  webp: ["tool.trim", "tool.recolor"],
  gif: ["tool.trim", "tool.recolor"],
  avif: ["tool.trim", "tool.recolor"],
  tiff: ["tool.trim", "tool.recolor"],
  tif: ["tool.trim", "tool.recolor"],
  ico: ["tool.trim", "tool.recolor"],
  heic: ["tool.trim", "tool.recolor"],
};

for (const [ext, expected] of Object.entries(expectedTools)) {
  const drawn = wheel
    .getPetalsForPage("tools", [ext], 8, true, manifests)
    .map((p) => p.id);

  check(`${ext}: tools page draws`, drawn, expected);

  // The whole point: what is drawn is what hover can reach.
  const hoverable = wheel.filterActions(manifests, "tools", [ext], true, 8).map((a) => a.id);
  check(`${ext}: hover list`, hoverable, expected);

  if (drawn.length !== hoverable.length || drawn.some((id, i) => id !== hoverable[i])) {
    failures.push(
      `${ext}: drawn and hoverable disagree\n      drawn   ${JSON.stringify(drawn)}\n      hover   ${JSON.stringify(hoverable)}`
    );
  }

  if (wheel.hasToolsForExtensions([ext], true, manifests) !== expected.length > 0) {
    failures.push(`${ext}: hasTools disagrees with the drawn tools`);
  }

  // Every drawn petal must be reachable somewhere in the ring.
  const centre = 100;
  const reached = new Set();
  for (let a = 0; a < 360; a += 1) {
    for (const r of [60, 85, 110, 124]) {
      const rad = (a * Math.PI) / 180;
      const idx = wheel.hitTestWedge(
        centre + r * Math.cos(rad),
        centre + r * Math.sin(rad),
        drawn.length,
        centre,
        centre,
        1
      );
      if (idx !== null && drawn[idx]) reached.add(drawn[idx]);
    }
  }
  for (const id of expected) {
    if (!reached.has(id)) failures.push(`${ext}: ${id} is drawn but can never be hovered`);
  }
}

// Video has no tools at all, and the page must say so rather than lie.
check(
  "mp4: no tools",
  wheel.getPetalsForPage("tools", ["mp4"], 8, true, manifests).map((p) => p.id),
  []
);
check("mp4: hasTools", wheel.hasToolsForExtensions(["mp4"], true, manifests), false);

rmSync(out, { recursive: true, force: true });

if (failures.length) {
  console.error(`\n${failures.length} failure(s):\n`);
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log(`✓ tools gating: ${Object.keys(expectedTools).length} formats, drawn list matches hover list`);