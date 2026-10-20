<!-- bmad:context -->
<!-- Verified 2026-10-16 against 2b5d4a0. Managed by bmad-project-context; edits inside this block are replaced on refresh. Keep anything you want preserved outside the markers. -->

## weebo-skillsagent

One repo of Claude Code skills plus `wska`, the Rust CLI that links a chosen profile of
them into a project or into `~/.claude/`. A one-member Cargo workspace (`crates/wska/`),
driven by Task and mise. `readme.md` covers profiles, items and layout; `docs/content/docs/wska.md` is
the CLI and TOML reference.

## Policy

- Never hand-edit a skill under `skills/` named in `vendor.toml`'s `provides` — all 15 are
  today. It diverges the recorded hash, after which `wska sync` reports the skill Patched
  and refuses to update it ever again. Change it upstream, then `task skills:sync`.
- A skill authored here is one absent from every `provides` list, and is freely editable.
- Never hand-edit `_bmad/` or `.claude/skills/` — installer output, overwritten by
  `npx bmad-method install` and `task skills:install`.
- Never put repo rules in `CLAUDE.md`; put them here. All four of its blocks are rewritten
  by their owners — the weebo-base template, `rtk init`, the graphify installer.
- Files under `OWNED` in `.tasks/template.yaml`, `.hooks/*.sh` included, are overwritten
  wholesale by `task template:sync` — change them in `batleforc/weebo-base`.

## Where things are

- `docs/content/docs/superpowers/2026-10-15-wska-deferred-findings.md` — `wska` defects already triaged
  and deliberately left unfixed. Read it before reporting one.

## Running and verifying

- Exclude `skills/` from searches unless the task is about a vendored skill — it is 918 of
  994 tracked files.
- The pre-commit hook runs `task recu`, `task lint` (clippy `-D warnings`), both
  `.hooks/*.sh` scripts and gitleaks, so a commit fails on one new clippy warning or stray
  whitespace.

<!-- /bmad:context -->
