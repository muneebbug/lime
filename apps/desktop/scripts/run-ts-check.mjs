/**
 * Bundles a TypeScript check script with the project's own Vite, then runs it on
 * Node.
 *
 * The checks import from `src`, which is a React app with JSX and CSS imports.
 * Node's `--experimental-strip-types` erases types but cannot load JSX or
 * resolve extensionless specifiers, so a check that touches app code needs a real
 * bundler. Vite is already a dev dependency here, so this leans on it rather than
 * adding Vitest or esbuild to the tree.
 *
 * Usage: node scripts/run-ts-check.mjs scripts/check-tools.mts
 */
import { build } from "vite";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const entry = process.argv[2];
if (!entry) {
  console.error("Usage: node scripts/run-ts-check.mjs <entry.mts>");
  process.exit(1);
}

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/*
 * The bundle is written inside the project, not into the OS temp dir.
 *
 * `external` packages are resolved at runtime by Node from the *bundle's*
 * location, and a temp directory has no node_modules above it - so `react` could
 * not be found. Keeping the output under the project root gives it the same
 * resolution the app itself gets. The path is removed afterwards, and `dist` is
 * already gitignored so a failed run cannot leave something committable behind.
 */
mkdirSync(join(root, "dist"), { recursive: true });
const outDir = mkdtempSync(join(root, "dist", ".ts-check-"));

/**
 * Left external so the bundle can load them from the project's node_modules.
 *
 * Only the modules a check could plausibly pull in through an app import are
 * listed. Bundling them would work too, but keeping them external means the
 * output stays small and the resolution stays the same one the app gets.
 */
const EXTERNAL = [
  "react",
  "react-dom",
  "motion/react",
  "lucide-react",
  "zustand",
  "@tauri-apps/api/core",
  "@tauri-apps/api/event",
  "@tauri-apps/api/webviewWindow",
];

function cleanup() {
  try {
    rmSync(outDir, { recursive: true, force: true });
  } catch {
    // The temp dir is disposable; a failed cleanup is not worth failing over.
  }
}

await build({
  root,
  logLevel: "error",
  build: {
    write: true,
    outDir,
    emptyOutDir: true,
    lib: {
      entry: resolve(root, entry),
      formats: ["es"],
      fileName: "check",
    },
    cssCodeSplit: false,
    minify: false,
    rollupOptions: { external: EXTERNAL },
  },
});

const child = spawn(process.execPath, [join(outDir, "check.js")], { stdio: "inherit" });
child.on("exit", (code) => {
  cleanup();
  process.exit(code ?? 1);
});
