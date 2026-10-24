---
name: weebo-deps
description: House dependency hygiene for Weebo projects (Rust + pnpm UI + mise tools) — bump dependencies in range and major by major, check that everything is up to date, show which CVEs a bump closed or opened (before/after snapshot diff), and remove unused dependencies (cargo machete, knip), with the `task deps*` contract. Use when asked to update packages/dependencies, check whether dependencies are current or unused, handle a Dependabot/updatecli PR batch, or before a release.
---

# Weebo dependency hygiene

This skill answers "is everything current, is everything used, and did the bump make CVEs better
or worse?". What to *do* about a CVE (triage ladder, no-suppression rule, exceptions) belongs to
[`weebo-security`](../weebo-security/SKILL.md); its `task audit` stays the gate.

## 1. Taskfile contract

Copy `templates/.tasks/deps.yaml` and include it with `flatten: true`
(`deps: { taskfile: .tasks/deps.yaml, flatten: true }`). Drop the `ui` lines when there is no UI.

| Task | Does | Fails on |
| --- | --- | --- |
| `deps` | `deps:outdated` + `deps:unused` + current advisories; writes nothing | an unused dependency |
| `deps:outdated` | `cargo update --dry-run --verbose`, `pnpm outdated`, `mise outdated --bump` | never (report) |
| `deps:unused` | `cargo machete`, `pnpm exec knip --dependencies` | a declared dependency nothing uses |
| `deps:update` | snapshot → `cargo update`, `pnpm update`, `mise:lock:update` → `lint` + `test` → snapshot → diff | a broken build/test, or an advisory the bump **introduced** |
| `deps:cve:snapshot LABEL=…` | `cargo audit --json` + `pnpm audit --json` → `.task/deps/cve-<LABEL>.txt` | an audit that did not run (no network, no lockfile) |
| `deps:cve:diff [FROM=before] [TO=after]` | prints `FIXED` / `INTRODUCED` / `REMAINING` | any `INTRODUCED` line |
| `deps:cve:branch [BASE=origin/main]` | snapshots BASE's lockfiles then the branch's, and diffs them; restores the lockfiles even on failure | uncommitted lockfile edits (refuses to run), any `INTRODUCED` line |

A snapshot line is `<source> <advisory id> <package>`. `source` is `rust`, `rust-unmaintained`,
`rust-unsound`, `rust-yanked` or `ui-<severity>`, so warnings move in the diff as well as
vulnerabilities. Versions are left out on purpose: going from 1.0 to 1.1 with the same advisory
still open is not a fix.

## 2. The update procedure

Follow it in this order; each step is one commit (`chore(deps): …`, or `fix(deps): …` when it
closes an advisory).

1. **Baseline.** `task deps`. Note the unused deps and every major reported as held back.
2. **Unused first.** Remove what `deps:unused` reports before bumping: fewer deps, less to bump.
   A false positive goes in `[package.metadata.cargo-machete] ignored = [...]` (or knip's
   `ignoreDependencies`) with the reason in a comment. Never ignore without the reason.
3. **In-range bump.** `task deps:update`. It only moves lockfiles and mise tools within their pins,
   so it should be green; if a test breaks, `cargo update -p <crate> --precise <old>` the culprit
   and report it rather than pinning silently.
4. **Majors, one at a time.** For each crate/package held back: read its changelog/release notes,
   `task deps:cve:snapshot LABEL=before`, edit the requirement (`Cargo.toml` — the
   `[workspace.dependencies]` entry when there is one — or `pnpm add <pkg>@latest` in `ui/`), fix
   the build and tests, then `task deps:cve:snapshot LABEL=after && task deps:cve:diff`.
   A major that introduces an advisory is not merged.
5. **Gate.** `task audit` (weebo-security) and `task deps` both green.
6. **Report** in this shape:

```
Bumped in range: 14 crates, 9 npm packages, 2 mise tools
Majors: axum 0.8 → 0.9 (done), vite 7 → 8 (held: plugin X not compatible yet, issue #…)
Removed (unused): serde_yaml, lodash
CVE diff: FIXED rust RUSTSEC-2026-0012 h2 · INTRODUCED none · REMAINING ui-moderate GHSA-… esbuild (dev-only, no fix)
```

`REMAINING` lines go through the weebo-security triage ladder; they are not this skill's to
suppress.

## 3. CI

`templates/.github/workflows/deps.yaml` runs the same tasks:

| Job | Trigger | Runs | Gate |
| --- | --- | --- | --- |
| `unused` | PR touching manifests, lockfiles, `ui/src` | `pnpm install --frozen-lockfile`, `task deps:unused` | block |
| `cve-diff` | every such PR (Dependabot's included) | `task deps:cve:branch BASE=<PR base sha>`, output in the run summary | block on `INTRODUCED` |
| `outdated` | weekly, manual | `task deps:outdated` into the run summary | report |

The whole-tree CVE gate stays in weebo-security's daily workflows; `cve-diff` only answers
"did this PR make it better or worse?". Actions are pinned by SHA, tools come from `mise.toml`
through `jdx/mise-action`, as everywhere else.

## 4. knip

[knip](https://knip.dev) is the JS/TS counterpart of cargo machete. It starts from the entry points
(`package.json` scripts and `main`, plus plugins for vite, vitest, eslint, playwright, …), follows
the imports, and reports what nothing reaches. `--dependencies` keeps it to the `package.json`
findings: **unused** dependencies (declared, never imported) and **unlisted** ones (imported, not
declared, so they only work through a transitive install). Without the flag it also reports unused
files and exports, which is useful once but too noisy for a gate.

It reads `tsconfig.json` `paths`, so aliases like `@app/*` need no config. A real false positive
(a package only loaded by a string name, a CLI only called from CI) goes in `ui/knip.json`:

```json
{ "$schema": "https://unpkg.com/knip@6/schema.json", "ignoreDependencies": ["pkg"] }
```

with the reason in the PR, since JSON has no comments. knip needs `node_modules` (it loads the tool
configs), hence the install step in CI.

## 5. Rules

- **Latest without a CVE.** Every bump targets the latest version; a version held back carries its
  reason in a comment next to the requirement (and an issue link when it waits on upstream).
- **Never downgrade to make a gate green** — security outranks tidiness (weebo-security §3).
- **A snapshot that did not run is a failure, not "zero CVEs".** The task checks the JSON, keep it so.
- **Majors are never batched.** One major per commit, so `git bisect` and revert stay one step.
- **Bump what the tools don't see:** GitHub Actions SHAs and Containerfile base digests are moved by
  Dependabot and updatecli (`weebo-security`, `weebo-containerfile`), not by `deps:update`;
  check their open PRs as part of the same pass.
- **Workspace dependencies stay single-sourced:** a bump edits `[workspace.dependencies]`, never a
  member's own `version =` (weebo-good-practice).

## 6. Tooling

On top of what `weebo-security` pins (cargo-audit, cargo-deny, pnpm):

```toml
"cargo:cargo-machete" = "latest"   # unused Rust dependencies
jq = "latest"                      # parses the audit JSON in deps:cve:snapshot
```

`knip` is a devDependency of `ui/` (`pnpm add -D knip`), so its version lives in the lockfile and
`pnpm audit` covers it. `cargo update --dry-run --verbose` replaces cargo-outdated: built in, and it
names the available version of every crate held back by a requirement.

## Templates

```
templates/
├── .tasks/deps.yaml               deps, deps:outdated, deps:unused, deps:update, deps:cve:{snapshot,diff,branch}
└── .github/workflows/deps.yaml    unused, cve-diff (PR), outdated (weekly)
```
