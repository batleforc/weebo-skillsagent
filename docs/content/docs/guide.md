---
title: "wska — guide by use case"
---

Find your situation, run what it says. Every command is explained in full in
the [reference](./wska.md); this page only says which ones to string together.

Two ways to call the same CLI:

- **Inside this repo**: `task skills:<command>` (wraps `cargo run -p wska`,
  takes `PROFILE=a,b ITEM=x REPO=<url> SCOPE=project|user AGENT=claude|gemini|antigravity RUN_INSTALLERS=0`).
- **Anywhere else**: the `wska` binary. Installers only run with
  `--run-installers`; the Task wrappers pass it by default.

Contents:

- [Getting started](#getting-started) — this repo, a new project, a fresh clone
- [Changing what is installed](#changing-what-is-installed) — add, remove, replace, uninstall
- [Where it installs](#where-it-installs) — user scope, Gemini CLI, Antigravity
- [Keeping up to date](#keeping-up-to-date) — skills, binary
- [Working on skills](#working-on-skills) — edit, author, vendor, external, installer, agent, team repo
- [Automation](#automation) — CI gates, Eclipse Che, Windows
- [When something goes wrong](#when-something-goes-wrong)

## Getting started

### Work on this repo itself

```bash
task mise:install     # toolchain: Rust, Task, cocogitto, node, uv, graphify…
task init             # mise, plugins, git hooks, every item linked into this repo's .claude/
```

### Equip a new project

1. Install the binary once per machine:

   ```bash
   mise use -g github:batleforc/weebo-skillsagent
   # or: cargo install --git https://github.com/batleforc/weebo-skillsagent wska
   ```

2. From anywhere inside the project, preview then apply:

   ```bash
   wska list --profiles                                   # pick profiles
   wska install --profile weebo --dry-run
   wska install --profile weebo --run-installers          # clones the items repo on first use
   wska doctor                                            # tools each item needs
   ```

3. Commit the selection, ignore the links (they are absolute paths into your
   clone):

   ```text
   .claude/skills/
   .claude/agents/   # only if you keep no hand-written agents there
   ```

   ```bash
   git add .claude/wska.toml .gitignore
   ```

Installers (`graphify`, `bmad`, `rtk`, …) write into the project; commit or
ignore their output as each tool expects.

### Clone a project that already uses wska

```bash
wska install --run-installers    # bare: re-applies the committed .claude/wska.toml
wska doctor
```

## Changing what is installed

| I want to…                              | Run                                              |
| --------------------------------------- | ------------------------------------------------ |
| add a profile                           | `wska add --profile design`                      |
| add one skill no profile lists          | `wska add --item weebo-helm`                     |
| drop it again                           | `wska remove --item weebo-helm`                  |
| replace the whole selection             | `wska install --profile docs --item batlehub-design` |
| see what is selected                    | `wska list --recorded`                           |
| see what is actually linked             | `wska status`                                    |
| everything in the repo                  | `wska install --profile all`                     |
| uninstall everything wska linked        | `wska remove` every recorded name (see below)    |

- `install --profile/--item` is declarative: whatever it does not name is
  unlinked. Use `add`/`remove` to change one thing.
- `remove` only accepts names that are recorded. An item brought by a profile
  leaves only with its profile.
- To uninstall everything, `remove` every profile, item and repo that
  `list --recorded` shows. Installer items are not undone: wska never owned
  what their command wrote (undo them by hand, e.g. `claude plugin uninstall …`).
- `planning` and `game-design` both run the bmad installer with different
  modules: pick one.

## Where it installs

Every command takes `--scope` and `--agent`; each combination has its own
directory and its own `wska.toml`.

| Goal                                   | Flags                               | Directory                     |
| -------------------------------------- | ----------------------------------- | ----------------------------- |
| this project, Claude Code (default)    | —                                   | `<git root>/.claude/`         |
| every project on this machine          | `--scope user`                      | `$CLAUDE_CONFIG_DIR` or `~/.claude/` |
| this project, Gemini CLI               | `--agent gemini`                    | `<git root>/.gemini/`         |
| every project, Gemini CLI              | `--agent gemini --scope user`       | `~/.gemini/`                  |
| this project, Antigravity              | `--agent antigravity`               | `<git root>/.agents/`         |
| every project, Antigravity             | `--agent antigravity --scope user`  | `~/.gemini/antigravity/`      |

```bash
wska install --profile base --scope user --run-installers
wska install --agent gemini --profile weebo --run-installers
wska update  --agent gemini                    # re-applies .gemini/wska.toml only
```

- Gemini CLI and Antigravity get **skills only**. Agents and MCP servers are
  skipped with a reason; an installer runs only if `vendor.toml` declares a
  setup for that agent (today: bmad, bmad-gds, graphify, rtk, ponytail-plugin,
  and mlab for Gemini CLI).
- Several agents in one project: run one install per `--agent`. Gemini CLI
  also reads `.agents/skills/`, so with both it sees each skill twice — same
  links, harmless.
- User and project scope stack: Claude Code reads both. Put the always-wanted
  skills (`base`) at user scope, the project-specific ones at project scope.
- Task wrappers take it as `AGENT`: `task skills:install AGENT=gemini PROFILE=base`.

## Keeping up to date

| What                          | Run                                                         |
| ----------------------------- | ----------------------------------------------------------- |
| skills, from the items repo   | `wska update` (git pull --ff-only, then re-apply `wska.toml`) |
| skills, user scope            | `wska update --scope user`                                  |
| the binary                    | `mise up github:batleforc/weebo-skillsagent`, or re-run the `cargo install` line |
| vendored skills (maintainer)  | `task skills:sync`, review `git diff skills/`, commit       |

Links are live: an edit pulled into the clone applies immediately; `update`'s
re-install is what picks up items a profile gained or lost. `update` fails and
installs nothing if the clone has local commits — see
[troubleshooting](#when-something-goes-wrong).

## Working on skills

### Edit a skill and see it everywhere

Point wska at a clone you edit instead of its own:

```toml
# the consuming project's mise.toml
[env]
WSKA_REPO = "{{env.HOME}}/git/weebo-skillsagent"
```

Then `wska install` once; the links now target your clone, so every edit there
is live in every project linked to it. Commit and push from the clone. Editing
through a project's `.claude/skills/<name>/` edits the same files — on purpose
or not at all.

### Write a new skill

1. `skills/<name>/SKILL.md` with frontmatter `name` and `description` (what
   the harness triggers on).
2. List it in a profile in `profiles.toml` — or leave it out and install it
   with `--item`.
3. Tools it shells out to go in `vendor.toml` `[requires]`.
4. A row in the readme's Items table.
5. `task skills:add ITEM=<name>` and `task skills:doctor`.

### Vendor a skill from an upstream repo

```toml
# vendor.toml
[sources.someone]
upstream = "https://github.com/someone/skills"
ref      = "main"
subpath  = "skills"
provides = ["the-skill"]       # prefer a list over "*"
licence  = "MIT"
```

```bash
task skills:sync SOURCE=someone   # copies it under skills/, records commit + hash
git diff skills/                  # review: this is unreviewed upstream content
```

Then profile, `[requires]` and readme row as for a new skill. An upstream laid
out as agents or rules uses `copies` instead of `subpath`/`provides`; a licence
outside `subpath` goes in `LICENSES/<source>.txt`.

### Patch a vendored skill

Edit it in `skills/`. `sync` sees the hash differ, reports it `Patched` and
never overwrites it. To take upstream again, revert your edit (`git checkout
skills/<name>`) and sync.

### Use a skill from a repo without vendoring it

`[external.<item>]` in `vendor.toml` (`upstream`, `ref`, `path`, `licence`):
install clones it into `.task/wska-cache/external/` and links it. Nothing is
hashed or reviewed — pin a tag as `ref` when that matters.

### Ship a tool, plugin or MCP server installed by a command

An `[installers.<item>]` entry with `command` and `scope` (`project`, `user`
or `any`), plus `gemini`/`antigravity` setups if those agents should get it.
No directory under `skills/`. The command runs with the target's parent as
working directory and `WSKA_REPO` set to the declaring repo. See the reference
for the plugin and `uv tool` examples.

### Ship a Claude agent

`agents/<name>/<name>.md`, listed in a profile like any item; it links into
`.claude/agents/<name>/`. Claude Code only.

### Share a team's own skills

Create a repo with the same layout (`profiles.toml`, `skills/`, optional
`agents/`, `vendor.toml`), then in each project:

```bash
wska add --repo https://github.com/my-team/team-skills --profile team
```

Names must be unique across all repos. Its installers run its commands — add
only repos you trust. `update` pulls it; `remove --repo <url>` drops it and its
links.

## Automation

### CI gates

```bash
wska doctor          # exit 1 if an item cannot work here
wska sync --check    # exit 1: missing licence, 2: a vendored skill is behind
task audit           # includes sync --check
```

A CI job that needs the skills runs the bare `wska install` (add
`--run-installers` if it needs graphify & co.) against the committed
`wska.toml`.

### Eclipse Che / no root

No package manager: everything comes from mise (`mise use -g
github:batleforc/weebo-skillsagent`, then `mise use <tool>` for what `wska
doctor` reports missing). The `weebo-cde` skill (profile `weebo`) covers
devfile sidecars for the rest.

### Windows

A `x86_64-pc-windows-msvc` binary ships with each release. It needs:

- **Developer Mode** (or an elevated shell), or creating links fails;
- **`HOME`** set — wska does not read `USERPROFILE`
  (`setx HOME %USERPROFILE%`, Git Bash sets it already);
- **`sh` on PATH** (Git for Windows) to run installers.

## When something goes wrong

| Symptom                                                         | Fix |
| --------------------------------------------------------------- | --- |
| `refuse …: a real file or directory is already there`           | a hand-copied skill sits where the link goes: move or delete it, re-run |
| `status` says `Foreign`                                         | a link into another clone (e.g. an old `WSKA_REPO`): delete it, re-run |
| `status` says `Dangling`                                        | item renamed or deleted, or an external's cache missing: bare `wska install` |
| `status` says `WrongTarget`                                     | bare `wska install` relinks it |
| bare `install` errors: nothing recorded                         | first install needs `--profile` or `--item` |
| `update` fails on `git pull --ff-only`                          | the clone has local commits: push or reset them in the clone (`$WSKA_REPO` or `~/.local/share/wska/weebo-skillsagent`) |
| `error: no profiles.toml in <dir>`                              | `WSKA_REPO` points at a non-items directory: fix or unset it |
| `item `x` … already defined by another items repo`              | two repos define one name: rename one or drop the repo |
| installer failed mid-install                                    | fix the cause, re-run: install converges |
| `doctor`: `missing: <tool>`                                     | `mise use <tool>`; Python modules via `pip`/`uv` |
| a skill is linked but never triggers                            | `wska doctor` checks its `SKILL.md` has `name` and `description` |
| `remove` refuses a name                                         | it comes from a profile: remove the profile, or `install` a new selection |

Still stuck: [known limits](./wska.md#known-limits) and the
[deferred findings](./superpowers/2026-10-15-wska-deferred-findings.md).
