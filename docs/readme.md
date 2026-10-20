# Documentation

Plain markdown, no static site — this is a single-CLI repo and a reference page
plus the design history is the whole of it. Switch to
[fumadocs](https://fumadocs.dev/) if that ever stops being true.

- [`guide.md`](guide.md) — wska by use case: new project, fresh clone,
  user scope, Gemini CLI / Antigravity, editing, authoring or vendoring a
  skill, team repos, CI, Windows, troubleshooting.
- [`wska.md`](wska.md) — CLI and config reference. Every command and its exit
  codes, using it from another project, cutting a release, the ownership rules that decide what `wska` will and will not touch,
  the `profiles.toml` and `vendor.toml` schemas, and how `sync` treats a locally
  patched skill.
- [`superpowers/`](superpowers/) — how the spine was built, kept for the
  reasoning rather than as instructions:
  - [`specs/2026-10-15-skills-spine-design.md`](superpowers/specs/2026-10-15-skills-spine-design.md)
    — the design and the decisions behind it, notably why distribution is
    symlinks rather than a plugin marketplace.
  - [`plans/2026-10-15-wska-skills-spine.md`](superpowers/plans/2026-10-15-wska-skills-spine.md)
    — the 13-task implementation plan it was built from.
  - [`2026-10-15-wska-deferred-findings.md`](superpowers/2026-10-15-wska-deferred-findings.md)
    — findings raised in review and deliberately left unfixed. Read this before
    filing a bug; it is probably already in here.
  - [`weebo-base-hooks-ignore.patch`](superpowers/weebo-base-hooks-ignore.patch)
    — the change the `weebo-base` template needs so its commit hooks stop
    reformatting vendored skills.

For the repo itself — what it holds, the profiles, the quick start — see the
[root readme](../readme.md).
