---
title: "Documentation"
description: "wska, judge and how this repo was built."
---

A [Fumadocs](https://fumadocs.dev/) site themed by
[`@weebo/operator-docs`](https://github.com/batleforc/weebo-operator-kit/tree/main/packages/operator-docs),
the format shared by the weebo docs sites. The pages are plain markdown under
`docs/content/docs/`; links between them stay relative `.md` links.

Published at <https://batleforc.github.io/weebo-skillsagent/> by
[`docs.yml`](https://github.com/batleforc/weebo-skillsagent/blob/main/.github/workflows/docs.yml)
on every push to `main` that touches `docs/`. Locally: `task docs:dev`
(<http://localhost:3000>), `task docs:build` for the static export.

- [Guide](./guide.md) — wska by use case: new project, fresh clone,
  user scope, Gemini CLI / Antigravity, editing, authoring or vendoring a
  skill, team repos, CI, Windows, troubleshooting.
- [Reference](./wska.md) — CLI and config reference. Every command and its exit
  codes, using it from another project, cutting a release, the ownership rules that decide what `wska` will and will not touch,
  the `profiles.toml` and `vendor.toml` schemas, and how `sync` treats a locally
  patched skill.
- [judge](./judge.md) — the headless review gates `rfc-ready` and `doc-review`.
- Design history — how the spine was built, kept for the reasoning rather than
  as instructions:
  - [Skills spine design](./superpowers/specs/2026-10-15-skills-spine-design.md)
    — the design and the decisions behind it, notably why distribution is
    symlinks rather than a plugin marketplace.
  - [Skills spine plan](./superpowers/plans/2026-10-15-wska-skills-spine.md)
    — the 13-task implementation plan it was built from.
  - [Deferred findings](./superpowers/2026-10-15-wska-deferred-findings.md)
    — findings raised in review and deliberately left unfixed. Read this before
    filing a bug; it is probably already in here.
  - [`weebo-base-hooks-ignore.patch`](https://github.com/batleforc/weebo-skillsagent/blob/main/docs/content/docs/superpowers/weebo-base-hooks-ignore.patch)
    — the change the `weebo-base` template needs so its commit hooks stop
    reformatting vendored skills.

For the repo itself — what it holds, the profiles, the quick start — see the
[root readme](https://github.com/batleforc/weebo-skillsagent#readme).
