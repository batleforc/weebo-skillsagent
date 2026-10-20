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

**Agent** — `--agent claude|gemini|antigravity` (default `claude`, accepted by
every command) picks whose directory the scope resolves to:

| agent         | `project`   | `user`                  |
|---------------|-------------|-------------------------|
| `claude`      | `.claude/`  | `$CLAUDE_CONFIG_DIR` or `~/.claude/` |
| `gemini`      | `.gemini/`  | `~/.gemini/`            |
| `antigravity` | `.agents/`  | `~/.gemini/antigravity/` |

Gemini CLI and Antigravity read the same `SKILL.md` folders, so skills link
as-is. Agents and MCP servers are Claude Code formats: for the other two
agents they are skipped with a reason. An installer runs for them only when
`vendor.toml` declares a setup for that agent (see `[installers.<item>]`):
today bmad, bmad-gds, graphify, rtk and ponytail-plugin for both, and
mlab (MCP) for Gemini CLI only. Each agent's directory holds
its own `wska.toml`, so `wska install --agent gemini` with no profile re-applies
the Gemini selection only. Gemini CLI also reads `.agents/skills/`, so in a
project linked for both, it sees each skill twice; the copies are the same
links, so it does not matter which one wins.

Target directories are canonicalized before anything is compared, so a
symlinked `~/.claude` still matches the links written through it.

## Commands

### `install`

```bash
wska install [--profile <name>…] [--item <name>…] [--repo <git url>…] [--scope project|user] [--dry-run] [--run-installers]
task skills:install [PROFILE=base,planning] [ITEM=weebo-techdoc] [REPO=<git url>] [SCOPE=project|user] [RUN_INSTALLERS=0]
```

Links every item named by the given profiles plus every `--item`, and removes
links it owns that are *not* in that set — so an install is a declaration of
the whole desired state for that target, not an addition to it.

`--profile` and `--item` are both repeatable. `--item` is how a skill that no
profile lists gets installed; an unknown name is an error. `--repo` adds an
[extra items repo](#extra-items-repos). The task wrapper takes comma-separated
lists.

Every applied install records what it was given in the target's
[`wska.toml`](#the-target-file-wskatoml). With neither `--profile` nor
`--item`, install re-applies that file instead, and errors if there is none —
a bare install never means "unlink everything". `--dry-run` prints the plan
and writes nothing, the file included.

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

### `add`

```bash
wska add [--profile <name>…] [--item <name>…] [--repo <git url>…] [--scope project|user] [--dry-run] [--run-installers]
task skills:add [PROFILE=design] [ITEM=weebo-techdoc] [REPO=<git url>] [SCOPE=project|user] [RUN_INSTALLERS=0]
```

`install` with the target's recorded [`wska.toml`](#the-target-file-wskatoml)
plus the given profiles and items, so adding one skill does not mean
re-typing everything already there. Names already recorded are not
duplicated; with no `wska.toml` yet it behaves like `install`. At least one
`--profile`, `--item` or `--repo` is required. The result is written back, so the next
bare `install`/`update` keeps it.

```bash
wska add --item weebo-techdoc    # same as: wska install --profile base --item weebo-techdoc
```

### `remove`

```bash
wska remove [--profile <name>…] [--item <name>…] [--repo <git url>…] [--scope project|user] [--dry-run] [--run-installers]
task skills:remove [PROFILE=design] [ITEM=weebo-techdoc] [REPO=<git url>] [SCOPE=project|user] [RUN_INSTALLERS=0]
```

The inverse of `add`: installs the recorded `wska.toml` minus the given
profiles and items, which unlinks what only they brought, and writes the
result back. At least one name is required, and every name must be recorded
there — otherwise it refuses and changes nothing. That catches the case that
would otherwise silently do nothing: an item that comes from a recorded
profile stays linked as long as the profile does, so it can only leave with
that profile (or by replacing the selection with `install`).

Removing the last name unlinks every wska link in the target and records an
empty selection; a bare `install` then errors until something is added.

Like every install, dropping an installer item does not uninstall what its
command wrote.

### `update`

```bash
wska update [--profile <name>…] [--item <name>…] [--scope project|user] [--run-installers]
task skills:update [PROFILE=base,planning] [ITEM=weebo-techdoc] [SCOPE=project|user] [RUN_INSTALLERS=0]
```

Fast-forwards the items repo (`git pull --ff-only`) — whichever one `wska`
resolved: `$WSKA_REPO`, the repo you are in, or its own clone (see [Using wska
in another project](#using-wska-in-another-project)) — and every [extra items
repo](#extra-items-repos) the target's `wska.toml` names. A local commit or a
diverged branch makes the pull fail and nothing is installed.

It then runs `install` with the given profiles and items, or, given none,
with the target's `wska.toml`. That re-install picks up items added to or
removed from a profile upstream. With nothing given and no `wska.toml`, the
pull is all it does — links are live, so pulled edits to existing skills
already apply.

Passing profiles or items replaces the recorded selection (use `add` to
extend it), with the same
declarative rule as `install`: whatever is not named is unlinked. There is no
`--dry-run`.

### The target file: `wska.toml`

Written next to the links, at `<target>/wska.toml` (`.claude/wska.toml` for
the project scope), by every applied install:

```toml
# Written by `wska install`. Run `wska install` or `wska update` with no
# --profile/--item to re-apply it; commit it so other machines can.
profiles = ["base", "docs"]
items = ["batlehub-design"]
repos = ["https://github.com/my-team/team-skills"]
```

`repos` is only written when there is one (see [Extra items
repos](#extra-items-repos)).

It holds what was asked for, not the expanded item list, so a profile that
gains an item upstream brings it on the next bare `install`/`update`. Edit it
by hand if you like; the next bare install applies it. `--run-installers` is
not recorded — pass it each time.

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
wska list [--profiles | --recorded] [--scope project|user]
task skills:list [PROFILES=1 | RECORDED=1] [SCOPE=project|user]
```

A table. With no flag, every item, its kind (`skill`, `agent`, `mcp`,
`installer`), whether it is installed in the `--scope` target, and the
profiles that list it directly — `-` for an item no profile lists, which only
`--item` installs:

```
ITEM                 KIND       INSTALLED  PROFILES
batlehub-design      skill      -          design
graphify             installer  recorded   base
judge                installer  -          planning, review
ponytail             skill      yes        base
weebo-techdoc        skill      -          docs
```

`INSTALLED` is `yes` for a correct link and `-` for nothing there. An
installer is never `yes`: wska cannot see what its command wrote, so it reads
`recorded` when the target's `wska.toml` selects it. Anything else is the
[`status`](#status) state of a path that needs a look: `wrong-target`,
`dangling`, `foreign`, `real-path`.

`--profiles`: every profile, what it includes and what it lists itself.

```
PROFILE      INCLUDES  ITEMS
base         -         wska, graphify, rtk, ponytail, ponytail-plugin, grill-me, grilling
docs         base      weebo-techdoc
```

`--recorded`: the target's [`wska.toml`](#the-target-file-wskatoml), each
profile expanded to every item it brings through its includes. Errors when
nothing is recorded.

```
NAME             RECORDED AS  ITEMS
docs             profile      graphify, grill-me, grilling, ponytail, ponytail-plugin, rtk, weebo-techdoc, wska
batlehub-design  item         batlehub-design
```

### `doctor`

```bash
wska doctor [--scope project|user]
task skills:doctor [SCOPE=project|user]
```

The scope only picks whose `wska.toml` names the [extra items
repos](#extra-items-repos) to check as well; the items repo itself is always checked.

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

### `completions`

```bash
wska completions <bash|zsh|fish|elvish|powershell>
```

Prints a shell completion script. Works outside this repo and never clones it.

```bash
wska completions bash > ~/.local/share/bash-completion/completions/wska
wska completions zsh  > "${fpath[1]}/_wska"
wska completions fish > ~/.config/fish/completions/wska.fish
```

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

## Using wska in another project

`wska` reads items from one place and writes links to another. **The target**
is always resolved from the working directory (`--scope project`) or the user
config dir (`--scope user`). **Items** come from the first of:

1. `$WSKA_REPO`, if set — cloned there first if the directory does not exist;
2. the git root of the working directory, if it holds a `profiles.toml` (you
   are inside this repo);
3. a clone wska owns at `$XDG_DATA_HOME/wska/weebo-skillsagent` (else
   `~/.local/share/wska/weebo-skillsagent`), made from
   `https://github.com/batleforc/weebo-skillsagent` on first use.

So a fresh machine needs only the binary:

```bash
mise use -g github:batleforc/weebo-skillsagent       # release asset
cargo install --git https://github.com/batleforc/weebo-skillsagent wska   # or build from source
```

then, from anywhere inside the other project:

```bash
wska install --profile base --profile planning --run-installers   # clones on first run
wska install --profile base --profile planning --item weebo-techdoc   # plus one skill no profile there lists
wska status
```

Set `WSKA_REPO` only to use a clone you edit yourself (for example in the
project's `mise.toml`: `[env] WSKA_REPO = "{{env.HOME}}/git/weebo-skillsagent"`).

Gitignore the links, because they are absolute paths into the clone and mean
nothing on another machine, but commit `.claude/wska.toml`:

```gitignore
.claude/skills/
```

Each machine then re-runs a bare `wska install --run-installers` after
cloning, which applies the committed selection. Installer items (`bmad`,
`graphify`, …) run with that project as working directory, so their output
belongs to the project and is committed or ignored as that tool expects.

Updating skills is a bare `wska update`: a `git pull --ff-only` of the clone,
then a re-install of `wska.toml` to pick up items added upstream. wska never
pulls except through `update`. Updating the binary is re-running the install line above.

A `WSKA_REPO` pointing at an existing directory with no `profiles.toml` stops
every command with `error: no profiles.toml in <dir>`; wska never clones into a
directory that already exists.

## Extra items repos

A target can draw from more than one items repo. `--repo <git url>` on
`install`, `add` or `remove` records it in the target's `wska.toml`:

```bash
wska add --repo https://github.com/my-team/team-skills --profile team
wska remove --repo https://github.com/my-team/team-skills --profile team
```

- Each extra repo must have the wska layout: a `profiles.toml`, and
  `skills/`, `agents/`, `mcp/` and an optional `vendor.toml`, like this one.
  It is cloned on first use into `$XDG_DATA_HOME/wska/repos/<url as a
  directory name>` (else `~/.local/share/wska/repos/…`), and pulled by `update`.
- Items and profiles of every repo share one namespace, so a profile in one
  repo may list items of another. A name defined by two repos stops every
  command with `item `<name>` in <repo> is already defined by another items repo`.
- Repos are a registry, not part of the choice: `install --profile …` keeps
  the recorded ones, and only `remove --repo` drops one. The items it brought
  are unlinked then, since every link into `wska/repos/` counts as wska's.
- An installer runs with `WSKA_REPO` set to the repo that declares it.
- `status`, `list` and `doctor` include the repos the target records.
  `sync` only ever refreshes the items repo itself.

Installers from an extra repo run commands that repo chose, so add only repos you trust.

## The `wska` skill

The repo ships a skill named `wska` (`skills/wska/SKILL.md`, in the `base`
profile) so that agents working in a target project go through this tool
instead of around it. It tells them to:

- never copy files into `.claude/skills/` or `.claude/agents/`, nor run
  `claude plugin install` / `claude mcp add` themselves — a command-driven
  item becomes an `[installers.<item>]` entry instead;
- prefer a bare `wska install`/`update`, which re-applies the target's
  `wska.toml`; when changing the selection, read that file and pass every
  profile and `--item` the target should keep, since install unlinks the rest;
- dry-run, then ask the user before running installers;
- run `wska doctor` afterwards and offer missing tools through mise;
- add a missing skill to this repo (`SKILL.md`, profile, `[requires]`, readme
  row) rather than inlining it in the project.

It is a skill, so it only reaches a project once `base` has been installed
there by hand the first time.

## Releasing

Versions come from the Conventional Commits history via cocogitto
(`tag_prefix = "v"`).

```bash
task release              # cog bump --auto: sets crates/wska version, updates Cargo.lock, changelog, commit, tag,
                          # then pushes the commit and the v* tag, which triggers .github/workflows/release.yml
```

The workflow runs the tests, then builds `wska` for `x86_64-unknown-linux-musl`,
`aarch64-unknown-linux-musl` and `aarch64-apple-darwin` and attaches each as
`wska-<target>.tar.gz` to the GitHub release for the tag. That naming is what
`mise use github:…` looks for.

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

Four independent tables.

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

An upstream that is not laid out as skills (rules, agents, references) uses
`copies` instead of, or beside, `subpath`/`provides`: each entry maps a local
directory (from this repo's root) to an upstream one (from the clone's root),
and is hashed, fast-forwarded and reported as patched exactly like a skill,
keyed by its local path. Both paths must be relative with no `..`.

```toml
[sources.skill-nr]
upstream = "https://github.com/Institut-du-Numerique-Responsable/skill-nr"
ref      = "main"
copies   = { "agents/nr-check" = "versions/claude-code/.claude/agents" }
licence  = "CC-BY-SA-4.0"
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
for it. `scope` is `project`, `user` or `any`: invoked at a scope it does not
admit, it is skipped with a reason. `any` runs at whichever scope `install` is
invoked with, for a machine-wide tool a project-scoped item needs (`judge`, a
`uv tool`). The command runs with its working directory set to the parent of
the target's `.claude/`, and with `WSKA_REPO` set to this repo's root so it
can reference files here:

```toml
[installers.judge]
command = 'uv tool install --force --editable "$WSKA_REPO/judge"'
scope   = "any"
```

A Claude plugin is installed the same way — `claude plugin install` is
idempotent, and `marketplace add` is joined with `;` because it may fail when
the marketplace is already declared:

```toml
[installers.ponytail-plugin]
command = "claude plugin marketplace add DietrichGebert/ponytail --scope project; claude plugin install ponytail@ponytail --scope project"
scope   = "project"
```

An installer runs for Claude Code only, unless it declares a `gemini` or
`antigravity` setup: that agent's own command and scope, since they differ
(rtk sets up Gemini CLI globally only, so `--scope user`). Without one, the
item is skipped for that agent with a reason.

```toml
[installers.rtk]
command = "rtk init --auto-patch"
scope   = "project"
gemini      = { scope = "user", command = "rtk init -g --gemini --auto-patch" }
antigravity = { scope = "project", command = "rtk init --agent antigravity --auto-patch" }
```

Dropping an installer item from a profile does not uninstall it: `wska` never
owned what the command wrote.

### `[external.<item>]` — skills fetched from git, not vendored

```toml
[external.batlehub-rfc]
upstream = "https://github.com/batlehub/batlehub"
ref      = "main"                 # branch or tag
path     = ".claude/skills/rfc"   # the skill directory inside the repo
licence  = "Apache-2.0"
```

`install` shallow-clones the repo into `.task/wska-cache/external/<item>/` if
it is not already there (never on `--dry-run`), checks `<path>/SKILL.md`
exists, then links it like any vendored skill. The cache is inside the repo,
so ownership, `status`, `doctor` and the unlink pass treat it unchanged.

`sync` (or `sync <item>`) re-clones; `sync --check` only reports a missing
licence. Externals record no commit or hash, so drift is invisible and nothing
upstream is reviewed before it is linked — use a tag as `ref` when that
matters. A fresh checkout has no cache: links into it read `Dangling` until
the next install fetches again.

## Known limits

Recorded in [`2026-10-15-wska-deferred-findings.md`](superpowers/2026-10-15-wska-deferred-findings.md).
The two worth knowing while using the tool:

- **Apply is not atomic.** Installers run before unlinks, so a failing installer
  leaves links created and stale links not yet removed. Re-running converges.
- **`sync` prints Rust `Debug`**, so its rows read as
  `FastForward { skill: "impeccable", from: "8a0e198", to: "4809728" }`.
  Cosmetic.
