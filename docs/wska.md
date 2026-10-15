# wska — reference

`wska` links this repo's items into a Claude Code target. One binary,
`crates/wska`, wrapped by the tasks in `.tasks/skills.yaml`.

Every task is a thin wrapper: `WSKA` defaults to `cargo run --quiet -p wska --`,
so `task skills:build` and `WSKA=./target/release/wska task skills:status` use
the compiled binary instead.

## Concepts

**Item** — one thing installable into a target, named by its directory. Items
are discovered by scanning `skills/`, `agents/` and `mcp/` at the repo root; a
directory per item, dotfiles and plain files ignored. A name may be claimed only
once across the three kinds.

Items come in two flavours:

- **Linked** — a directory symlinked into the target. Edits here are live
  immediately in every target.
- **Installer** — an item that cannot be symlinked because it writes
  project-coupled state. Declared under `[installers]` in `vendor.toml` with a
  command `wska` shells out to.

**Profile** — a named set of items in `profiles.toml`, optionally including
other profiles. `all` is reserved and expands to every item in the repo;
declaring a profile named `all` is an error.

**Scope** — which target to write into. `project` resolves to the `.claude/` of
the git root containing the working directory (falling back to the working
directory itself when there is no `.git`, which keeps the CLI usable in a plain
directory). `user` resolves to `$CLAUDE_CONFIG_DIR`, else `$HOME/.claude`.

Target directories are canonicalized before anything is compared, so a
symlinked `~/.claude` still matches the links written through it.

## Commands

### `install`

```bash
wska install --profile <name> [--profile <name>…] [--scope project|user] [--dry-run] [--run-installers]
task skills:install PROFILE=base [SCOPE=project|user] [RUN_INSTALLERS=0]
```

Links every item named by the given profiles, and removes links it owns that
are *not* in that set — so an install is a declaration of the whole desired
state for that target, not an addition to it.

`--profile` is required and repeatable. `--dry-run` prints the plan and writes
nothing.

Installers are skipped unless `--run-installers` is passed. The task wrapper
passes it by default; `RUN_INSTALLERS=0` opts out and leaves those items
installed nowhere.

Output is one line per action (`link`, `unlink`, `skip`, `warn`) plus a summary:

```
warn   webapp-testing: missing py:playwright
link   …/.claude/skills/frontend-design -> …/skills/frontend-design
skip   graphify: installers run only with --run-installers
linked 4 unlinked 0 installers 0 skipped 1
```

`warn` is advisory and never blocking: a skill whose tools are absent is still
readable.

Running install twice changes nothing the second time.

### `status`

```bash
wska status [--scope project|user]
task skills:status [SCOPE=project|user]
```

One row per item in the repo, naming what is at its place in the target:

| State                | Meaning                                                       |
| -------------------- | ------------------------------------------------------------- |
| `Linked`             | our link, pointing at the expected item                       |
| `WrongTarget`        | our link, pointing at a different item in this repo           |
| `Dangling`           | our link, whose target under this repo no longer exists       |
| `Foreign`            | a symlink pointing outside this repo — never touched          |
| `RealPath`           | a real file or directory — never touched                      |
| `InstallerUnmanaged` | an installer item; `wska` cannot inspect what it installed    |
| `MissingFromTarget`  | nothing is there                                              |

`InstallerUnmanaged` is not a problem — it is the honest answer for an item
whose installation `wska` does not own.

### `list`

```bash
wska list [--profiles]
task skills:list [PROFILES=1]
```

Item names, or profile names with `--profiles`. One per line.

### `doctor`

```bash
wska doctor
task skills:doctor
```

Asks, per item, whether it could actually work on this machine:

- every requirement in `vendor.toml`'s `[requires]` resolves, and
- the item really is a usable skill — a `SKILL.md` whose YAML frontmatter
  carries a non-empty `name` and `description`. Those two are what the harness
  reads to list and trigger a skill, so a copy missing either is inert however
  cleanly it synced.

A requirement is satisfied when any of its `|`-separated alternatives resolves,
which is how one entry covers a Chrome-ish browser across distributions. An
alternative written `py:<module>` is an importable Python module rather than a
command — the only honest check for a skill that drives a library no PATH lookup
can prove is installed.

```
frontend-design              ok
webapp-testing               missing: py:playwright
1 item(s) cannot work here
```

Exits 1 if anything is unhealthy, which makes it usable as a gate.

### `sync`

```bash
wska sync [source…] [--check]
task skills:sync [SOURCE=impeccable]
task skills:sync:check
```

Refreshes vendored skills from the upstreams in `vendor.toml`. With no source
named, every source. Each source is shallow-cloned into `.task/wska-cache/`
(gitignored), then compared skill by skill against three things: the local tree,
the hash recorded in `vendor.toml`, and upstream.

| Outcome          | Condition                                            | Effect                                      |
| ---------------- | ---------------------------------------------------- | ------------------------------------------- |
| `Unchanged`      | local matches its recorded hash, and matches upstream | nothing                                     |
| `FastForward`    | local matches its recorded hash, upstream has moved  | local replaced by upstream, hash advanced   |
| `Patched`        | local does *not* match its recorded hash             | never overwritten; recorded hash kept       |
| `MissingLicence` | the source records no `licence`                      | reported                                    |

The recorded hash is what makes a local edit safe: a patched skill is detected
and left alone, and the source's `commit` is not advanced while any of its
skills is patched — the tree genuinely is not at that commit.

`--check` writes nothing and reports, and its exit code separates an unmet
obligation from an available update:

- `0` — clean
- `1` — a source has no licence recorded (fix this)
- `2` — a skill is behind upstream (there is an update to review)

`task audit` runs `sync --check` after `cargo audit`, so a vendored skill
arriving without a licence fails the audit.

After a sync, read `git diff skills/` before committing — upstream content lands
in your tree unreviewed.

### Errors

Any command exits 1 with `error: <message>` on failure. The specific one worth
knowing: install refuses to touch paths it does not own, printing each before it
stops.

```
refuse …/.claude/skills/impeccable: a real file or directory is already there
error: refusing to change 1 path(s) this tool does not own
```

Nothing is written when a plan is refused — the refusal happens before any
action. Move or delete the path yourself, then re-run.

## Ownership model

`wska` only ever modifies a symlink whose resolved target lies inside this
repository. Everything else at a path it wants is reported and left alone.

The resolution is lexical (`.` and `..` folded without touching the
filesystem) and handles a relative link against the link's own directory, so a
link written by hand still classifies correctly, and `…/repo/../outside` is
correctly seen as leaving the repo.

This is the whole safety story: a target's hand-written skills and skills linked
from somewhere else survive any `wska` command, including the unlink pass of an
install that no longer wants that name.

## `profiles.toml`

```toml
[profiles.<name>]
include = ["<other profile>", …]   # optional, resolved recursively
items   = ["<item name>", …]       # optional
```

Unknown profile, unknown item, an include cycle, or a profile named `all` are
all errors naming the offender. Scope is never recorded here, so one profile
serves both scopes.

## `vendor.toml`

Three independent tables.

### `[sources.<name>]` — where a vendored skill came from

```toml
[sources.anthropic-skills]
upstream = "https://github.com/anthropics/skills"
ref      = "main"
subpath  = "skills"                 # directory within the clone holding the skills
provides = ["mcp-builder", …]       # a named list, or "*" for every directory in subpath
licence  = "Apache-2.0"
commit   = "dbd4588"                # written back by sync
hashes   = { mcp-builder = "sha256:…" }   # written back by sync
```

`commit` and `hashes` are maintained by `sync` — treat them as output. `provides`
must be `"*"` or a list; anything else is rejected with the offending value.

Prefer a named list over `"*"` when the upstream repo ships more than you want:
the current Anthropic source also carries `docx`, `pdf`, `pptx`, `xlsx`,
`skill-creator` and `claude-api`, which the harness already provides as
built-ins.

A licence text that lives outside `subpath` upstream cannot be carried in by
sync, and adding it to the skill directory by hand would make the local hash
diverge from the recorded one — marking the skill `Patched` forever. Put it in
`LICENSES/<source>.txt` instead.

### `[requires]` — external commands per item

```toml
[requires]
bmad            = ["npx", "uv"]
frontend-design = ["google-chrome|chromium|chrome"]
webapp-testing  = ["python3", "py:playwright"]
```

Hand-maintained; `wska` reads it and infers nothing. Alternatives separated by
`|` are satisfied when any one resolves. `py:<module>` is an import check rather
than a PATH lookup. Naming an item that does not exist in the repo is an error.

### `[installers.<item>]` — items installed by a command

```toml
[installers.graphify]
command = "graphify install --project --platform claude"
scope   = "project"
```

The item exists by virtue of this entry — there is no directory under `skills/`
for it. `scope` says which scope it belongs to; invoked at the other one, it is
skipped with a reason. The command runs with its working directory set to the
parent of the target's `.claude/`.

## Known limits

Recorded in [`2026-10-15-wska-deferred-findings.md`](superpowers/2026-10-15-wska-deferred-findings.md).
The two worth knowing while using the tool:

- **Apply is not atomic.** Installers run before unlinks, so a failing installer
  leaves links created and stale links not yet removed. Re-running converges.
- **`sync` prints Rust `Debug`**, so its rows read as
  `FastForward { skill: "impeccable", from: "8a0e198", to: "4809728" }`.
  Cosmetic.
