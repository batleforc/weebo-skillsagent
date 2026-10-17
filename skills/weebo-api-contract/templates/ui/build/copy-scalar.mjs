// source: batlehub@20879e5 (ui/build/copy-scalar.mjs), trimmed.
//
// Copy the Scalar API-reference bundle into the build output, so `/scalar` is
// served from this origin instead of a CDN. Run after `vite build`:
//   "build": "vue-tsc -b && vite build && node build/copy-scalar.mjs"
//
// Why: utoipa-scalar's stock template loads an unversioned CDN script on the
// origin that holds the console's tokens; it is a blank page with no egress; and
// the bundle's supply chain was invisible to `pnpm audit` and the SBOM. As a
// declared devDependency it is covered by all three.
//
// Which artifact: `dist/browser/standalone.js`, the self-contained IIFE. Not
// `standalone.esm.js`, which code-splits into `chunks/` and would need every
// chunk copied and a `type="module"` tag.

import { copyFileSync, mkdirSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const uiRoot = resolve(here, "..");

const SOURCE = resolve(uiRoot, "node_modules/@scalar/api-reference/dist/browser/standalone.js");

// Must match SCALAR_BUNDLE_PATH in the backend. Under `assets/`, which the SPA
// fallback treats as build-owned: a missing file stays a 404 instead of
// falling through to `index.html`.
const TARGET = resolve(uiRoot, "dist/assets/scalar/standalone.js");

let source;
try {
  source = statSync(SOURCE);
} catch {
  console.error(
    `copy-scalar: ${SOURCE} not found.\n` +
      "Run `pnpm install` first — @scalar/api-reference is a devDependency.",
  );
  process.exit(1);
}

mkdirSync(dirname(TARGET), { recursive: true });
copyFileSync(SOURCE, TARGET);

const kb = Math.round(source.size / 1024);
console.log(`copy-scalar: assets/scalar/standalone.js (${kb} kB)`);
