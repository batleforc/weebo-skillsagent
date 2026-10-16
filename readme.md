# weebo-skillsagent

One repository holding the Claude Code skills used across every project, plus
`wska` — the CLI that links a chosen subset into a project or into the user's
global `~/.claude/`.

It replaces [`batleforc/weebo-skills`](https://github.com/batleforc/weebo-skills),
which published skills as a plugin marketplace and is abandoned.

## How it works

A skill is a plain directory containing a `SKILL.md`. `wska install` symlinks
the directories named by a profile into the target's `.claude/skills/`, so
editing a skill here is live in every target immediately, with no reinstall
step.

Two items cannot be symlinked, because each writes state coupled to the project
it is installed in: `bmad` renders per-project snapshots from an
installer-generated `_bmad/config.toml`, and `graphify` is installed by pipx and
writes its own skill directory. Both are declared as installers in
`vendor.toml`, and `wska` shells out to them instead of linking.

## Quick start

Requires the toolchain from `mise.toml` (`task mise:install`), which brings
Rust, Task, cocogitto and most of what the items themselves need: `node`, `uv`,
`graphify`, `gitleaks`, `cargo-audit`.

```bash
task init                                         # full bootstrap: mise, plugins, hooks, all items into this repo
task skills:install PROFILE=design                # one profile into this repo's .claude/
task skills:install PROFILE=frontend SCOPE=user   # the same into ~/.claude/
task skills:status                                # what is linked where
task skills:doctor                                # can each item actually work on this machine
```

Install is idempotent — run it twice and the second run changes nothing — and it
never writes over a path it does not own. A real directory, or a symlink
pointing outside this repo, is refused and named rather than replaced.

Installers run by default from the Task wrapper; pass `RUN_INSTALLERS=0` to link
only. The bare CLI is the other way round: installers are skipped unless you
pass `--run-installers`.

## Profiles

Declared in [`profiles.toml`](profiles.toml). Scope is never recorded there, so
one profile serves both `--scope project` and `--scope user`.

| Profile    | Includes  | Adds                                       |
| ---------- | --------- | ------------------------------------------ |
| `base`     | —         | `graphify`, `rtk`, `ponytail`, `grill-me`, `grilling` |
| `planning` | `base`    | `bmad`, `batlehub-rfc`                     |
| `design`   | `base`    | `frontend-design`, `theme-factory`, `impeccable`, `batlehub-design`, `monofolio-design` |
| `frontend` | `design`  | `webapp-testing`                           |
| `mcp`      | `base`    | `mcp-builder`                              |
| `game-design` | `frontend` | `bmad-gds`, `blender-image-to-3d`, `3dviz-pro-max` |

`game-design` carries BMGD (bmad's game dev studio module) as its own item,
`bmad-gds`, because the bmad installer takes `--modules` as the whole install:
that command names `bmm,gds` so BMM survives. Installing `planning` and
`game-design` together runs that installer twice — pick one.

Its two 3D voices are deliberate, the way `design` carries three UI ones:
`blender-image-to-3d` is a gated reference-image-to-game-asset pipeline,
`3dviz-pro-max` is broader scene and Three.js work. The latter is 12 MB of
catalog and kit templates — drop it from the profile if your project has no use
for it.

`base` carries the four things every project gets: the graph (`graphify`), the
output filter (`rtk`), lazy-dev mode (`ponytail`) and the grilling interview.
`grill-me` is a three-line shim whose body is "call the Skill tool with
grilling", so the pair travels together. Ponytail's five companion skills are
vendored but not in `base` — add the one you want to a profile.

Upstream ships ponytail as a Claude plugin whose hooks arm ponytail mode at
session start. Vendoring carries the skills only: `ponytail` is invoked as a
skill rather than armed by a hook, and `ponytail-gain` has no benchmark data to
read. Install the plugin instead if you want the hook.

`design` overlaps deliberately: `frontend-design` and `theme-factory` are
references, `impeccable` is opinionated. Loading all three gives three voices
that can disagree — trim to the one you trust per project.

`PROFILE=all` is reserved and means every item in the repo; a profile may not be
named `all`.

## Items

| Item              | Installs by | Provenance                                    | Needs on PATH                       |
| ----------------- | ----------- | --------------------------------------------- | ----------------------------------- |
| `graphify`        | installer   | `graphify install --project --platform claude` | `graphify`                          |
| `bmad`            | installer   | `npx bmad-method install`                     | `npx`, `uv`                         |
| `frontend-design` | symlink     | vendored from `anthropics/skills`             | a Chrome-ish browser                |
| `theme-factory`   | symlink     | vendored from `anthropics/skills`             | —                                   |
| `mcp-builder`     | symlink     | vendored from `anthropics/skills`             | `python3`, `npx`                    |
| `webapp-testing`  | symlink     | vendored from `anthropics/skills`             | `python3`, Playwright for Python    |
| `impeccable`      | symlink     | vendored from `pbakaus/impeccable`            | `node`, `npx`, a Chrome-ish browser |
| `bmad-gds`        | installer   | `npx bmad-method install --modules bmm,gds`   | `npx`, `uv`                         |
| `blender-image-to-3d` | symlink | vendored from `majidmanzarpour/blender-game-skills` | `blender`, `python3`, Pillow  |
| `3dviz-pro-max`   | symlink     | vendored from `viettranx/3dviz-pro-max`       | `python3`, `node`, Playwright or a browser |
| `rtk`             | installer   | `rtk init --auto-patch`                       | `rtk`                               |
| `ponytail`        | symlink     | vendored from `DietrichGebert/ponytail`       | —                                   |
| `ponytail-audit`, `-debt`, `-gain`, `-help`, `-review` | symlink | vendored from `DietrichGebert/ponytail` | — |
| `grill-me`, `grilling` | symlink | vendored from `mattpocock/skills`             | —                                   |
| `batlehub-design` | symlink     | snapshot of `batlehub/batlehub`'s design system | —                                 |
| `batlehub-rfc`    | symlink     | derived from `batlehub/batlehub`'s rfc skill + template, with security blocks | —   |
| `monofolio-design`, `batlehub` | installer | Claude plugins from `batleforc/weebo-skills` | `claude`                    |

Requirements are hand-maintained in `vendor.toml`'s `[requires]` table — `wska`
reads it, it does not infer anything — so an upstream that starts shelling out
to something new goes unnoticed until that table is updated. `task
skills:doctor` checks the table against this machine.

Most of those tools come from `mise.toml`. The ones that do not are a Chrome-ish
browser, Blender (`blender-image-to-3d` also accepts `$BLENDER_BIN`, which
`doctor` cannot see) and Playwright for Python — install the latter with `pip install
playwright && playwright install chromium`. `doctor` can see the Python module
but not the browser binaries it downloads, so a green `webapp-testing` row is
necessary and not sufficient.

Vendored skills are checked in under `skills/`, with their upstream commit and a
content hash recorded in `vendor.toml`. `task skills:sync` fast-forwards them;
`task skills:sync:check` reports drift without writing. Licence texts that do
not live inside the vendored subtree are kept in `LICENSES/`.

## Layout

```
skills/              vendored and authored skills, one directory each
crates/wska/         the CLI
profiles.toml        named sets of items
vendor.toml          provenance, installers, per-item tool requirements
.tasks/skills.yaml   the task wrappers
docs/                documentation
LICENSES/            licence texts for vendored skills that need them
```

`agents/` and `mcp/` are reserved. `wska` already scans both and dispatches on
item kind, so adding an agent is content plus one enum variant, not a new
mechanism. MCP needs config merging into `.mcp.json` and is specified but not
built.

`.claude/skills/` is gitignored — it is generated output, the links `wska`
writes into this repo as its own target.

## Documentation

- [`docs/wska.md`](docs/wska.md) — CLI and config reference: every command, the
  ownership model, both TOML schemas, sync semantics, exit codes.
- [`docs/superpowers/`](docs/superpowers/) — the design spec, the implementation
  plan, and the findings deliberately left unfixed.

## Development

```bash
task skills:test   # cargo test -p wska
task lint          # cargo fmt --check, clippy -D warnings
task audit         # cargo audit, then vendored drift and licence check
```

Commits follow [Conventional Commits](https://www.conventionalcommits.org),
enforced by cocogitto (`cog.toml`). The pre-commit hook runs `task recu`, `task
lint`, gitleaks and the whitespace scripts; the commit-msg hook runs `cog
verify`. Commits must be signed and signed-off by the repository owner.

## Licence

This repository is [Apache-2.0](LICENSE.md). Vendored skills keep their own
licence, recorded per source in `vendor.toml` — `Apache-2.0` or `MIT`
depending on the source.
