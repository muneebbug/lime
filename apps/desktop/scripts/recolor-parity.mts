/**
 * Checks the frontend's colour maths against the Rust engine.
 *
 * The Recolor window previews on a canvas using its own port of the engine's
 * maths (`recolorMath.ts`), and the saved file is produced by the engine itself
 * (`recolor.rs`). That is duplication, and duplication of floating-point colour
 * conversion is exactly the kind that drifts: a changed coefficient, a different
 * rounding rule, a missing clamp. When it drifts the preview becomes a lie, and
 * nothing else in the build would catch it.
 *
 * So the engine writes a fixture of inputs and its own outputs
 * (`cargo test -p wheel-engines parity_fixture -- --ignored`), and this replays
 * every case through the frontend's implementation and demands byte equality.
 *
 * Run: `pnpm --filter desktop run test:parity`
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
// The extension is required: Node resolves ESM specifiers literally, so the
// extensionless form Vite accepts does not work here.
import { applyRecolor, type RecolorParams } from "../src/lib/recolorMath.ts";

interface FixtureCase {
  name: string;
  params: RecolorParams;
  input: number[];
  expected: number[];
}

const here = dirname(fileURLToPath(import.meta.url));
const fixture = JSON.parse(
  readFileSync(join(here, "..", "src", "lib", "__tests__", "recolor-parity.json"), "utf8")
) as FixtureCase[];

if (fixture.length === 0) {
  console.error(
    "The fixture is empty. Regenerate it with:\n" +
      "  cargo test -p wheel-engines parity_fixture -- --ignored"
  );
  process.exit(1);
}

let failures = 0;

for (const testCase of fixture) {
  // `Uint8ClampedArray`, not a plain array: that is what `ImageData.data` is,
  // and it wraps out-of-range writes instead of throwing. Using the same type
  // means the parity check exercises the real clamping behaviour.
  const actual = new Uint8ClampedArray(testCase.input);
  applyRecolor(actual, testCase.params);

  const expected = testCase.expected;

  if (actual.length !== expected.length) {
    console.error(
      `✗ ${testCase.name}: length ${actual.length}, expected ${expected.length}`
    );
    failures++;
    continue;
  }

  // Report the first divergence plus how many there are, rather than all of them:
  // a single rounding difference produces one byte, but a wrong tolerance curve
  // produces thousands and printing them all buries the useful information.
  const mismatches: number[] = [];
  for (let i = 0; i < expected.length; i++) {
    if (actual[i] !== expected[i]) mismatches.push(i);
  }

  if (mismatches.length === 0) {
    console.log(`✓ ${testCase.name}`);
    continue;
  }

  failures++;
  const first = mismatches[0];
  const pixel = Math.floor(first / 4);
  const channel = ["r", "g", "b", "a"][first % 4];

  console.error(`✗ ${testCase.name}: ${mismatches.length} of ${expected.length} bytes differ`);
  console.error(
    `  first at byte ${first} (pixel ${pixel} channel ${channel}): ` +
      `got ${actual[first]}, engine says ${expected[first]}`
  );
  console.error(
    `  pixel in:  [${testCase.input
      .slice(pixel * 4, pixel * 4 + 4)
      .join(", ")}]`
  );
  console.error(
    `  pixel out: [${actual.slice(pixel * 4, pixel * 4 + 4).join(", ")}] ` +
      `(engine: [${expected.slice(pixel * 4, pixel * 4 + 4).join(", ")}])`
  );
  console.error(
    "  The frontend preview and the Rust engine have diverged. Update " +
      "recolorMath.ts to match recolor.rs, then regenerate the fixture."
  );
}

if (failures > 0) {
  console.error(`\n${failures} of ${fixture.length} cases diverged from the engine.`);
  process.exit(1);
}

console.log(`\nAll ${fixture.length} cases match the Rust engine.`);
