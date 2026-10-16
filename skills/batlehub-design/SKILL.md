---
name: batlehub-design
description: Baseline design system for Weebo / BatleHub UIs — "The Proof Sheet" bitmap type specimen, OKLCH tokens in dark and light renditions, Silkscreen + JetBrains Mono, hairline rules, resolution as state. Use when building, styling or reviewing any BatleHub UI, or any project that adopts this design system as its baseline.
---

# BatleHub design system (baseline)

A snapshot of the design system from [batlehub/batlehub](https://github.com/batlehub/batlehub)
(Apache-2.0). It is the baseline: a project may add surfaces and components, but keeps these tokens
and Named Rules. Anything a project changes is written into that project's own `DESIGN.md`, not
left implicit.

## Files

| File | What it is |
| --- | --- |
| `DESIGN.md` | **The source of truth.** Frontmatter = tokens (colors, typography, spacing, components); body = rationale and every Named Rule. Read it before any UI work. Impeccable-format, so the impeccable skill reads it as-is. |
| `tokens.css` | The same tokens as CSS custom properties: dark on `:root`, light on `:root[data-theme="light"]`. |
| `reference/index.html` | The built proving surface every value was recorded from. Open it to compare your output against the real thing. |
| `reference/fonts/` | Self-hosted Silkscreen 400/700 and JetBrains Mono (OFL-1.1). |
| `reference/halftone-plate.png` | The system's only image, an authored 60 lpi halftone plate. |

## Applying it

1. Copy `tokens.css` and `reference/fonts/` into the project; declare the faces as
   `reference/index.html` does (lines with `@font-face`).
2. Components ask for token names (`var(--accent)`), never for a rendition. Theme is a stored
   preference (System / Light / Dark) resolved to `data-theme` before first paint.
3. Build from `DESIGN.md`'s `components:` frontmatter, then check the result against its
   "Do's and Don'ts".

## Non-negotiables (summary — `DESIGN.md` has the full text)

- **Integer Em**: Silkscreen only at multiples of 8px (16/24/40/56/72/88/104). All data, numbers and
  prose are JetBrains Mono.
- **No radius, no shadow at rest, no cards, no tiles, no glow.** Regions are separated by full-width
  hairline rules (dashed at rest, solid under state), never by inset containers.
- **Spacing** only from s1–s6: 4 / 8 / 12 / 16 / 24 / 40px.
- **Crimson (`--accent`) is the only invented colour** and stays on its jobs; a crimson fill always
  takes `--accent-ink`. Copper is second; everything else is ink and rule.
- **Three Channel**: every state is pattern + word + hue together, never hue alone.
- **Resolution as state**: verified = fine 3×3 dots, full ink; not = coarse 2×2 dots, copper/dim/crimson.
  The **only** animation is a row resolving from blurred to crisp.
- **Rule beside its row**: a denial explains its policy in the row of the thing it refused.
- **Body never scrolls horizontally**; wide content scrolls inside its own container.

## Provenance

Snapshot of upstream commit `2336695` (`DESIGN.md`, `docs/.vitepress/theme/tokens.css`,
`ui/design-proof/`). Refreshed by hand — see the comment in this repo's `vendor.toml`.
