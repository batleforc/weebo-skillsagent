---
title: "weebo-hud — status line and resume pane"
---

`weebo-hud` has two parts:

- **A status line** under the prompt shows the session (folder, model, context, quotas) and the machine (free RAM, CPU, free disk). It is an ANSI powerline drawn by `wska hud statusline`, in the style of the old `~/.claude/statusline.sh`.
- **A resume pane** beside the chat holds what you need to pick a session back up at a glance: the goal, the plan, where things stand and what Claude is waiting on from you. A Claude Code mod (a plugin) provides it, and Claude keeps it current itself.

It is built for the terminal, in local sessions and in Eclipse Che workspaces. The mod's source lives in `mods/weebo-hud/`; the root `.claude-plugin/marketplace.json` publishes it as `weebo-hud` in the marketplace `weebo`. The status line lives in `crates/wska/src/hud/`. The design and its rationale are in [the spec](superpowers/specs/2026-10-24-weebo-hud-design).

Why two parts: a plugin cannot set Claude Code's main `statusLine`, and a band drawn by a mod sits above the prompt, not under it. So `wska` draws the line and writes the setting, and the mod keeps the pane.

```text
 weebo-skillsagent  ★ Opus 5.5  ctx 34%  5h 62%  14:30  7j 21%  sam 09:00  RAM 22.9/32.0G libre  CPU 12%  disque 120G libre
```

## Install

```bash
task skills:install ITEM=weebo-hud SCOPE=user
```

That runs `wska hud install`, then `claude plugin marketplace add batleforc/weebo-skillsagent --scope user` and `claude plugin install weebo-hud@weebo --scope user`. The pane's hooks run in the current session right away, and in every new session after that. Run `/reload-plugins` in an open session.

**`wska hud` needs a wska release.** The installer first runs `wska hud --help`; when the `wska` on the PATH has no `hud` subcommand, it stops with "the wska on PATH has no 'hud' subcommand; upgrade it (mise upgrade) or release wska first" and installs nothing. Machines that install `wska` through mise only get `wska hud` after a wska release (`task release`). Until then, use the binary built from this repo (`cargo build -p wska`, or `cargo install --path crates/wska`).

**User scope only.** The status line is a user-wide setting, so a project install makes no sense. The installer is part of the `base` profile. Installing `base` at project scope skips it with the note "declared for User scope, invoked at Project"; it does not fail. Run the line above once per machine or workspace.

**Update:**

```bash
task skills:update ITEM=weebo-hud SCOPE=user   # marketplace update + plugin update
```

**Before the repo is pushed**, or to try a local change, add the working copy as a marketplace:

```bash
claude plugin marketplace add /path/to/weebo-skillsagent
claude plugin install weebo-hud@weebo --scope user
```

A marketplace that is a folder is read from that folder, so later edits only need `/reload-plugins`.

**What `wska hud install` does.** It writes this into `$CLAUDE_CONFIG_DIR/settings.json`, or `~/.claude/settings.json`:

```json
"statusLine": { "type": "command", "command": "wska hud statusline", "refreshInterval": 5 }
```

- The other keys and their order, the file mode and symlinks are kept. It never runs `jq`.
- **Retiring the old status line is automatic.** It replaces any previous `statusLine`. Before its first change it copies the file to `settings.json.weebo-hud-bak` and never overwrites an existing backup, so the old command is recoverable from there. You can delete `~/.claude/statusline.sh` yourself.
- When the setting is already there, it changes nothing.
- It fails on invalid JSON, or when neither `CLAUDE_CONFIG_DIR` nor `HOME` is set.
- `wska hud uninstall` removes the setting only when it is ours. Both commands take `--settings <path>`.

## The status line

Segments, left to right:

| Segment | Example | Source | Color |
| --- | --- | --- | --- |
| Folder | `weebo-skillsagent` | name of the working directory | blue |
| Model | `★ Opus 5.5` | `model.display_name` | violet |
| Context | `ctx 34%` | `context_window.used_percentage` | green, orange from 50 %, red from 70 % |
| 5-hour quota | `5h 62%  14:30` | `rate_limits.five_hour`, reset time | green, orange from 50 %, red from 80 % |
| 7-day quota | `7j 21%  sam 09:00` | `rate_limits.seven_day`, reset day and time | same as 5h |
| RAM | `RAM 22.9/32.0G libre` | free / total | green, orange below 20 % free, red below 10 % |
| CPU | `CPU 12%` | use since the previous refresh | green, orange from 80 %, red from 95 % |
| Disk | `disque 120G libre` | available space per filesystem | same as RAM |

The reset glyph before the time is U+F021 (a Nerd Font glyph). Claude Code refreshes the line every 5 seconds. Reset times are in local time, with French day names (`dim lun mar mer jeu ven sam`).

**Where the figures come from:**

- **Quotas.** The `rate_limits` of the JSON Claude Code sends on stdin. They exist for subscribers only, and only after the first response of a session. The command makes no network call and reads no token.
- **RAM.**
  - Under a cgroup v2 memory limit (a Che pod): free = limit minus the working set (used minus `inactive_file`). Reclaimable page cache counts as free, as in Kubernetes. Without that, a `cargo build` would keep the segment red.
  - Without a limit: `MemAvailable` / `MemTotal` from `/proc/meminfo`.
- **CPU.**
  - Under a cgroup quota: use relative to the quota, so 100 % means the pod's allotted cores are saturated. Otherwise: the whole machine, from `/proc/stat`.
  - A percentage needs two samples, so the previous one is kept in `$XDG_RUNTIME_DIR/weebo-hud-cpu`. The first run shows `—`.
- **Disk.** Read with `df`, with a 3 s deadline. Each distinct filesystem gets its own segment, labelled by its mount's last folder (`/` for the root), for example `disque projects 120G libre`; two paths on the same filesystem show once. A single filesystem has no label.
- **Platforms.** RAM and CPU are Linux-only: they show `—` on macOS and Windows. Disk (read with `df`) also shows `—` on Windows.

**Missing data shows a dash, never an error.** A figure that cannot be read shows `—` in that segment alone, and the rest of the line keeps working. The command never fails, whatever its input.

**Narrow terminals.** With every segment on, the line is about 150 columns wide. Turn off what you don't need (see Options).

## Options

Each segment is on unless its variable is `0`, `false`, `off` or `no`. Set them in the `env` block of `~/.claude/settings.json`, which Claude Code passes to the status line command:

```json
{
  "env": {
    "WEEBO_HUD_CPU": "0",
    "WEEBO_HUD_RESET7J": "off"
  }
}
```

| Variable | Controls |
| --- | --- |
| `WEEBO_HUD_DOSSIER` | folder segment |
| `WEEBO_HUD_MODELE` | model segment |
| `WEEBO_HUD_CONTEXTE` | context segment |
| `WEEBO_HUD_CINQH` | 5-hour quota segment |
| `WEEBO_HUD_SEPTJ` | 7-day quota segment |
| `WEEBO_HUD_RESET5H` | reset time on the 5h segment |
| `WEEBO_HUD_RESET7J` | reset day and time on the 7j segment |
| `WEEBO_HUD_RAM` | RAM segment |
| `WEEBO_HUD_CPU` | CPU segment |
| `WEEBO_HUD_DISQUE` | disk segment(s) |

The pane has one option left, `paneAuto` (boolean, on by default): it controls the automatic opening of the pane. Set it in `/config`; a change reloads the mod right away. Thresholds, colors, segment order and the 5 s interval are fixed.

## The resume pane

The pane is titled **Reprise** and is written in French:

```text
Objectif
  Livrer le HUD weebo
Plan
  ✓ Écrire la spec
  ▸ Implémentation de la tâche 5
  · Relecture finale
Notes
  Statusline terminée ; reste la doc du panneau.
En attente
  Valider le message de commit
```

| Block | What it holds | Who writes it |
| --- | --- | --- |
| Objectif | one line: the current goal | Claude, through `set_status` |
| Plan | Claude's task list: `✓` done, `▸` in progress (shown as its active form), `·` pending | mirrored from Claude's task tools, without any extra call |
| Notes | where things stand, decisions made | Claude, through `set_status` |
| En attente | what Claude needs from you (shown in orange, hidden when empty) | Claude, through `set_status` |

With nothing stored yet, the pane reads `Aucune note pour cette session.`

### How Claude fills it

- **Objectif, Notes and En attente.** The mod registers a tool, `mcp__weebo-hud__set_status`, which is always listed. Its description is the instruction: call it when work starts, after each step completed, and whenever you wait on the user. This repo's `CLAUDE.md` repeats that in its Skills section.
  - Input: `{ objectif?, notes?, attente? }`, all strings.
  - Each field given replaces the stored one, and an empty string clears it. There is no history.
- **Plan.** The task list mirrors itself.
  - `TaskCreate` adds an item, and `TaskUpdate` changes its status, subject or active form, or removes it.
  - `TodoWrite`, which `claude -p` and the SDK use, replaces the whole list.
  - Only successful calls count, and the mod never changes what those tools return.

### Which session it follows

- **Main conversation only.** Calls made inside a subagent are ignored, so a subagent's own sub-task never replaces your plan.
- **The current session.** The notes are saved per session id. They come back with `claude --resume`, and after a `/clear` or a `/resume` inside a session, the pane switches to that session's notes. Notes are never written under another session's id.
- **Kept 30 days.** Entries not updated for 30 days are deleted when a session starts.
- **Interactive sessions only.** Under `claude -p` the tool and `/etat` are not registered, so headless jobs get no extra tool.

### Opening and closing

- The pane opens on its own, without taking focus, the first time a session has content. Turn that off with the `paneAuto` option.
- It opens automatically at most once per session: if you close it, it stays closed.
- `/etat` opens it (with focus) or closes it.
- **Where it shows:** in the fullscreen layout from 110 columns, the pane docks beside the transcript; otherwise it sits above the prompt. An automatic opening is only placed from 144 columns and waits until there is room. `/etat` always shows it.

## Troubleshooting

**No status line, or `wska: command not found`.** `wska` must be on the PATH Claude Code runs with, which is not always your shell's PATH. Check with `command -v wska`, and that `statusLine` is in the settings file (`wska hud install` again is safe). If `wska hud` is an unknown subcommand, your wska predates it: mise-installed machines need a wska release first (`task release`); until then use the binary built from this repo.

**Reset times are an hour off, or in UTC.** They use the local time zone. In a minimal container without `/etc/localtime` they fall back to UTC: set `TZ`. A POSIX rule string such as `TZ=CET-1CEST,M3.5.0,M10.5.0/3` works without zoneinfo.

**5h and 7j show `—`.** No rate-limit reading has come in. That happens with an API key (no subscription quota) or before the first response of a session. It is expected, not a bug.

**RAM shows the host's memory in Che.** The pod has no cgroup v2 memory limit, or runs on cgroup v1. In both cases the figure falls back to `/proc/meminfo`, which reports the node.

**CPU shows `—`.** The first refresh has nothing to compare with (sessions share the state file, and a sample younger than 1 s is kept as the baseline, so rapid refreshes still compare against it), or the state file (`$XDG_RUNTIME_DIR/weebo-hud-cpu`, else `/tmp/weebo-hud-cpu-<uid>`) cannot be written, so the previous sample is lost.

**Two status lines, or the old one is back.** Only one `statusLine` can be set. Check `~/.claude/settings.json`, and recover the previous value from `settings.json.weebo-hud-bak` if needed.

**The pane doesn't show or stays empty.** Run `claude plugin list` and check that `weebo-hud@weebo` is enabled at user scope, then `/reload-plugins`; `claude --debug` logs any hook that failed. An empty pane means Claude has not called `set_status` yet and has no task list: ask it to update the resume pane. In a project without the `CLAUDE.md` line, the tool description alone drives the habit.

## For contributors

```text
mods/weebo-hud/
├── .claude-plugin/plugin.json   manifest: name, version, userConfig (paneAuto)
├── hooks/hooks.json             { "modules": ["./register.tsx"] }
├── hooks/register.tsx           engine wiring: pane render, set_status, /etat,
│                                task mirror, session tracking
├── hooks/notes.ts               loadNotes, mergeNotes, isEmpty, purgeKeys
├── types/index.d.ts             Notes, Todo and the PluginState contract
└── tests/pane.test.tsx          claude-code/testing: notes and pane

crates/wska/src/hud/             wska hud: statusline, install, uninstall
```

- **State.** `$.state` key `weebo-hud`, holding `{ notes }`. Handlers write it; renders only read.
- **Store.** One `$.store` entry per session: the key is `session:<id>`, and the value is `{ objectif, notes, attente, todos, updatedAt }`.
- **Status line.** Tested with `cargo test -p wska`.

Dev loop:

```bash
task mods:dev     # claude --plugin-dir mods/weebo-hud
task mods:test    # claude plugin validate, then tsc, then claude plugin test
```

The API reference is the `plugin-authoring` skill. The authoritative types are written by the engine to `mods/weebo-hud/.claude-plugin/types/` (gitignored) the first time it loads the mod. On a fresh clone, `mods:test` skips the tsc step with a warning ("types not generated yet: run `task mods:dev` once"); validate and the tests still run.
