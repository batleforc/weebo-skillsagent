# Skills spine — design

- Date: 2026-10-15
- Status: proposed
- Sub-project: A of three (see Scope)

## Intent

`weebo-skillsagent` becomes the single place that holds the skills, MCP
references and agents used across the author's projects, and installs a
chosen subset into any project or into the user's global Claude Code
configuration.

It replaces `batleforc/weebo-skills`, which is abandoned. The two plugins
that repo published (`monofolio-design`, `batlehub`) are out of date and
will not be carried over; new authored skills (`batlehub-design`,
`weebo-operator-kit`) will be written here later.

Success for this sub-project: from a clone, `task init` leaves every skill
in the repo linked into the repo's own `.claude/`, and
`task skills:install PROFILE=<p> SCOPE=user` links a named subset into
`~/.claude/`. Running either twice changes nothing.

## Scope

In scope (sub-project A):

- Repo layout for skills, and the slots agents and MCP will occupy.
- `profiles.toml` and `vendor.toml` formats.
- The `wska` CLI: `install`, `status`, `list`, `sync`.
- Vendoring the third-party skills currently installed as plugins.
- Task wiring and the migration off plugin-based installs.

Out of scope, each getting its own spec:

- Sub-project B — agents (commit agent, split review agents). The layout
  reserves `agents/` and the CLI dispatches on item kind, so B adds
  content and one kind, not a new mechanism.
- Sub-project C — MCP references. Reserves `mcp/`. C is where the CLI
  gains config merging into `.mcp.json` / `settings.json`.

## Decisions

### Distribution is symlinks, not a plugin marketplace

Skills live as plain directories in this repo and are symlinked into a
target's `.claude/skills/`. Editing a skill here is immediately live in
every target, with no reinstall step.

Verified 2026-10-15: Claude Code discovers skills reached through a
symlink. A temporary project containing only
`.claude/skills/zorblax-probe -> <outside path>` listed `zorblax-probe`
among its available skills under `claude -p`. This is an upstream
behaviour, not a contract; if it ever regresses, `claude --plugin-dir
<path>` loads a plugin from a local directory and is the fallback.

The layout stays harness-neutral — a skill is a directory with
`SKILL.md` and its references, with nothing Claude-Code-specific in it —
so adding a second harness later is a new link target, not a rewrite.

### Third-party skills are vendored

Copies live in this repo and are committed. This was chosen over pinning
upstream marketplaces for control, offline use and the ability to patch.
It costs: every upstream update is the author's to pull, and each vendored
skill carries its upstream licence.

Two exceptions are not vendorable, because they are installers rather than
content:

- `bmad` — each `SKILL.md` is a stub that runs
  `{project-root}/_bmad/scripts/render_skill.py`, and `_bmad/config.toml`
  is installer-generated with the project name baked in. A symlinked bmad
  skill in another project would look for an `_bmad/` that is not there.
- `graphify` — installed by `pipx:graphifyy`, which writes its own skill
  directory and a `.graphify_version`.

These are recorded as installers and run in the target, never linked.

### A Rust CLI, not a shell script

The install writes into `~/.claude/`, which holds the user's own
configuration, so the logic that decides whether a path may be replaced is
a trust boundary. `ln -sfn` aimed at a path that is already a symlink to a
directory creates the link *inside* the target rather than replacing it,
and this differs between GNU and BSD coreutils; a shell installer hitting
that case scatters links into a directory it does not own. Distinguishing
"a link I created" from "a directory the user wrote by hand" needs
`symlink_metadata` plus a link-target read, which is three lines in Rust
and fragile in shell.

Secondary reasons: `yq` is present on the author's machine but absent from
`mise.toml`, so a shell implementation would depend on undeclared tooling
and fail on the next machine partway through making links; sub-project C
needs JSON config merging that must not destroy keys it did not write,
which `serde_json` plus write-then-rename gets right by construction; and
the logic is testable in-process, which shell equivalents in practice are
not.

The bootstrap objection is answered by mise, which has `cargo`, `ubi`,
`github` and `http` backends — the binary becomes a declared, locked tool
like every other. Until a release exists, `cargo run --` works from the
repo.

Crate and binary are named `wska` (Weebo Skill Agent). Tasks wrap it, so
the binary name is rarely typed.

### The CLI is a linker, and stays one

In scope: resolve profiles, compute a plan, make and remove symlinks, run
installer commands, merge MCP config, report status and drift. All of that
concerns *where a file goes*.

Out of scope, permanently:

- **Not a skill runner.** It never reads or interprets `SKILL.md` beyond
  the frontmatter `name`.
- **Not a registry.** No publishing, no index, no "version 2.1 of
  ponytail" — vendoring means the version is whatever is committed here.
- **Not a dependency resolver.** Skills declare no needs on other skills;
  if one needs another, both go in the profile. Transitive resolution
  brings diamond conflicts and a solver.
- **Not a scaffolder.** Creating a skill from a template belongs to a
  skill (`skill-creator`), not the installer.
- **Not a config manager.** It owns the links it makes and the MCP entries
  it adds, not hooks, permissions or model settings.
- **No daemon or watcher.** Symlinks make edits live because the tool is
  not in the loop at runtime.

That last point is the reason for the boundary: this design is cheap
precisely because the tool has almost no behaviour after it exits. Version
resolution or dependency graphs would put `wska` between the author and
their own skills as something to debug.

The test for any future feature: does it require understanding *what a
skill means*, or only *where a file goes*? The second is in scope; the
first is not.

## Repo layout

```
skills/<name>/SKILL.md (+ references/, assets/, LICENSE)
agents/<name>.md                      reserved for sub-project B
mcp/<name>/                           reserved for sub-project C
profiles.toml
vendor.toml
crates/wska/                   the CLI
.claude/skills/                       symlinks into skills/ (gitignored)
```

`skills/` is flat: authored and vendored skills sit side by side, and
provenance lives in `vendor.toml` keyed by name rather than in the path.
Profiles reference skills by name, so if vendored skills lived under their
own subtree, promoting a patched skill to authored would change its path
and break every profile naming it. A skill is the author's own precisely
when no `vendor.toml` source claims it — no extra marker to maintain.

Item names must be unique across `skills/`, `agents/` and `mcp/`, because
a profile names items without stating their kind.

## `profiles.toml`

```toml
[profiles.base]
items = ["ponytail", "graphify"]

[profiles.rust-backend]
include = ["base"]
items = ["bmad"]

[profiles.frontend]
include = ["base"]
items = ["impeccable"]
```

- `include` composes profiles. There is no separate "global baseline"
  concept: a profile that others include provides that behaviour with one
  mechanism. The CLI attaches no meaning to the name `base`.
- `items` lists item names, not kinds. The CLI resolves each name and
  dispatches on its kind, so a profile expresses intent and the mechanism
  stays in one place.
- `include` cycles are rejected during planning.
- `all` is a reserved profile name meaning every item in the repo. It is
  what this repo installs into itself, so dogfooding needs no maintained
  profile.

Scope is never written in this file. The same profile is legitimately
wanted globally and in one project, so binding it here would force
duplicate profiles that drift.

## `vendor.toml`

```toml
[sources.superpowers]
upstream = "https://github.com/anthropics/claude-plugins-official"
ref      = "main"                     # branch, tag or commit
subpath  = "superpowers/skills"
provides = "*"                        # or an explicit list of names
licence  = "MIT"
commit   = "9f2a1c4"                  # what is currently vendored
hashes   = { brainstorming = "sha256:...", "writing-plans" = "sha256:..." }

[installers.bmad]
command = "npx bmad-method install"
scope   = "project"
```

Provenance is recorded per *source*, not per skill, because one upstream
repo usually carries many skills. `provides` maps a source to the skill
directories it populates; profiles still name individual skills, and the
CLI maps a skill back to its source.

`hashes` records the content hash of each vendored directory **as it came
from upstream**. Whether the author has patched a skill is derived from a
hash mismatch, never declared in a field. Hashes are per-skill so that
patching one skill does not freeze the rest of its source.

Directory hash definition, pinned because changing it later would mark
every skill as patched: SHA-256 over entries sorted by repo-relative path,
each contributing its path bytes, a NUL separator, its length and its
contents. File modes, timestamps and ownership are excluded. Symlinks
inside a vendored skill are hashed as their target text.

## CLI surface

```
wska install --profile <p>... [--scope project|user] [--dry-run]
wska status  [--scope project|user]
wska list    [--profiles]
wska sync    [<source>...] [--check]
```

### Targets

- `--scope user` → `$CLAUDE_CONFIG_DIR` if set, else `~/.claude`.
- `--scope project` → the git root's `.claude/`, falling back to the
  working directory.
- Within a target: `skills/` items link into `.claude/skills/`, `agents/`
  items into `.claude/agents/`. Missing target directories are created.

### Install is declarative

The profiles named on one invocation define the complete set for that
scope. Items the CLI owns in that scope which are not in the set are
unlinked. Multiple `--profile` flags union explicitly.

This is safe only because installs are links: removing a link the CLI
created destroys nothing, since the content lives in this repo. An
additive design would need a second verb to remove anything and would
leave the installed set as accumulated history that cannot be
reconstructed.

Every removal is printed. `--dry-run` prints the plan and writes nothing.

### Plan, validate, then apply

A command resolves its full plan before touching the filesystem: every
item name exists, `include` has no cycles, no name is ambiguous across
kinds, and no target path is occupied by something foreign. If validation
fails the command exits having changed nothing, so a typo in
`profiles.toml` cannot leave a half-installed scope.

A filesystem error during apply stops and reports what was already done.
Each link is individually reversible, so nothing needs rolling back by
hand.

Running `install` twice with the same arguments makes no changes and exits
0.

### Ownership, and the refusal rule

There is no lockfile. What the CLI owns is derived from the links
themselves: a target entry is owned when it is a symlink whose target
resolves inside this repository. Derived ownership cannot drift the way
recorded state does when a link is removed by hand.

Each target entry classifies as exactly one of:

| State | Meaning | Action |
|---|---|---|
| owned, correct | links to the right item here | left alone |
| owned, wrong target | links elsewhere in this repo | relinked |
| owned, dangling | stored target is under this repo, missing | relinked if the item is in the plan, removed if not |
| foreign link | symlink pointing outside this repo | refused, reported |
| real path | a directory or file, not a link | refused, reported |

A refusal names the path and the reason, and fails the plan before any
change is made. This is what keeps `install` from destroying a skill the
user wrote by hand into `~/.claude/skills/`.

A consequence worth knowing: two clones of this repo installing into the
same scope each see the other's links as foreign and refuse. That is
correct, and it will look like a bug the first time.

### Installers

Installer items run their `command` in the target project. They are not
convergent — bmad's installer writes real directories that the CLI did not
create and cannot meaningfully un-run — so:

- They execute on `install` and are **never** auto-removed.
- `status` lists them separately as *installed, unmanaged*.
- An installer whose declared `scope` does not match the invocation's
  scope is **skipped with a notice**, not treated as an error. bmad is
  project-coupled by construction, so `base` containing `graphify` must
  still install cleanly at `--scope user` — the alternative would force
  per-scope duplicate profiles, which is exactly what keeping scope out of
  `profiles.toml` exists to prevent.

The ownership rule already handles the namespace collision: bmad's real
`.claude/skills/bmad-prd/` directory classifies as a real path, so the CLI
leaves it alone rather than fighting the installer for the same directory.

### `status`

Per scope, reports: owned and correct, owned and dangling, foreign
(skipped, with the reason), installer-managed, and any item named by a
profile but absent from the target.

## Sync

`sync` fetches each source with `git clone --depth 1 --branch <ref>` into
a gitignored cache and copies `subpath`. Tags and commits are accepted as
`ref`. No sparse checkout.

For each skill a source provides, three values are compared: the recorded
hash, the local copy's current hash, and upstream's new content.

- Local matches recorded → unpatched → overwrite with upstream, update
  `commit` and the hash.
- Local differs from recorded → patched → **do not overwrite.** Print the
  diff between the vendored commit and the new upstream content, and leave
  the local copy alone.

This is deliberately not a merge engine. A three-way merge of another
author's prose skill is a judgement call; the useful thing a tool can do
is show exactly what upstream changed.

Skipping sync entirely was considered — re-vendor by hand when it matters.
It was rejected because without a recorded hash there is no way to
distinguish a local patch from stale vendored content, and that
distinction is the only thing preventing a future sync from silently
reverting the author's own edits.

Each vendored skill keeps upstream's `LICENSE` beside it and records an
SPDX id in `vendor.toml`. `sync --check` writes nothing and fails when a
source has no licence recorded, reporting also which sources are behind
upstream and which skills are patched. It is the audit entry point.

## Testing

Tests run over temporary directories, with no network.

- Profile resolution: `include` chains, cycle rejection, unknown item
  name, a name claimed by two kinds.
- Plan validation: a plan naming a foreign path fails before any link is
  made. Most cases live here; this is the data-loss guard.
- Ownership classification: one test per row of the table above.
- Convergence: install A, install B, assert A's links are gone and B's
  present; install B again, assert no changes.
- Directory hashing: stable across reruns, insensitive to mtime,
  independent of filesystem iteration order.
- Sync: a real git repository is created in a temporary directory and
  cloned over `file://`, exercising the actual clone path. Three cases —
  clean fast-forward, patched-copy refusal, missing licence.

Not automated: whether Claude Code discovers a symlinked skill. It needs a
model call, was verified once (see Decisions), and is recorded as a
verified assumption rather than paying for a model call per CI run.

## Task wiring

A new `.tasks/skills.yaml`, plus edits to the root `Taskfile.yaml`:

```
task skills:install PROFILE=<p> SCOPE=project|user
task skills:status
task skills:sync            task skills:sync:check
task lint   -> cargo fmt --check && rtk cargo clippy
task audit  -> rtk cargo audit && wska sync --check
task init   -> existing steps, then skills:install PROFILE=all SCOPE=project
```

`task init` gaining the self-install is what makes dogfooding automatic: a
clone plus `task init` leaves every skill linked locally.

`cargo-audit` is installed through mise's cargo backend
(`mise use cargo:cargo-audit`); this is unverified and is the first thing
to check when wiring `task audit`.

## Migration

Ordered, because step 4 is the point of no easy return.

1. Move `.claude/skills/graphify/` out of version control. `.claude/skills/`
   becomes gitignored wholesale, which also correctly ignores bmad's
   installer-created directories.
2. Drop `monofolio-design` and `batlehub`: they are out of date and will
   not be carried over. The `weebo` marketplace entry in
   `.claude/settings.json` is removed rather than repointed.
3. Vendor the third-party skills currently installed as plugins
   (`superpowers`, `ponytail`, `impeccable`) into `skills/`, with
   `vendor.toml` sources and licences, and record `bmad` and `graphify` as
   installers.
4. Remove the `ponytail`, `impeccable` and `superpowers` entries from
   `enabledPlugins` and their marketplaces from `extraKnownMarketplaces`
   in `.claude/settings.json`. Until this step the plugin path and the
   link path are both live for the same skill, which means two copies at
   possibly different versions. Do it only after step 3 is confirmed
   complete, and as its own commit.
5. Leave the `graphify hook-guard` hooks in `settings.json` untouched.
6. `.tasks/claude.yaml` stays — it is template-owned and will be restored
   by `task template:sync` regardless — but goes unused.

## Rejected alternatives

- **Pinning upstream marketplaces instead of vendoring.** Zero maintenance
  and no licence duty, but no control, no offline use, no patching, and no
  answer for installer-based skills. Rejected in favour of vendoring with
  explicit sync.
- **Taskfile plus shell installer.** Smaller, matches the repo's existing
  jq-driven idiom, no new toolchain. Rejected over the `ln -sfn` trap at a
  trust boundary, dependence on undeclared `yq`, untestability, and
  sub-project C's JSON merging.
- **chezmoi owning placement.** A real tool that already handles multiple
  machines. Rejected because it only pays off if chezmoi already manages
  `~/.claude`, and it would express profiles in chezmoi's model instead of
  the repo's.
- **A lockfile recording what was installed.** Rejected: ownership derived
  from link targets cannot drift, and needs no file.
- **Three separate distribution mechanisms for skills, agents and MCP.**
  Rejected: they differ in content, not in distribution.

## Risks

- Vendoring commits third-party content, so repo size and upstream drift
  are the author's from then on.
- Declarative install means `install --profile frontend` in a project set
  up with `rust-backend` unlinks the rust skills. Every removal is
  printed, but it will surprise once.
- `.claude/skills/` being gitignored means a fresh clone has no skills
  until the CLI runs — including for an agent working in this repo.
- The first deliverable is a tool, not skills: nothing is installable
  until the CLI's happy path works.
- Symlinked-skill discovery is upstream behaviour, verified once, not
  guaranteed.
