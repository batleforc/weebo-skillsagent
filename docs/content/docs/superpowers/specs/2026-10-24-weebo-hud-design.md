---
title: "weebo-hud — design"
---

- Date: 2026-10-24
- Status: approved (grilling session, 2026-10-24)
- Changes after review (2026-10-24): RAM under a cgroup counts `inactive_file` as free; Plan mirrors the task list (`TaskCreate`/`TaskUpdate`, or `TodoWrite`); disk segments are labeled when there are several; pane, tool and plan follow the main conversation and the current session only; auto-open once per session; tool and `/etat` registered in interactive sessions only.
- Replaces: `~/.claude/statusline.sh` (the settings `statusLine` command)
- Revised 2026-10-24: the status line moved into `wska hud statusline`; the band below is superseded (see the Revision section).

## Intent

A Claude Code mod that shows two things:

1. **The machine's free resources** (RAM, CPU, disk) in a powerline band, beside the session facts the old status line showed.
2. **A resume pane** that Claude keeps up to date, so the user can pick a session back up at a glance: the goal, the plan, where things stand and what Claude is waiting on.

## Scope

- **Terminal only.** It runs in local sessions (Fedora) and in Eclipse Che pods. It does not target VS Code, desktop or mobile, and nothing else is guaranteed to draw.
- **Shipped from this repo.** The root `.claude-plugin/marketplace.json` lists `mods/weebo-hud`. A wska installer `weebo-hud` installs it, in the same way as `diff-review`.

## Revision (2026-10-24): status line in wska

The live test showed the band drawn above the prompt. The user wants the classic `statusLine` back under the prompt, rendered as ANSI, with no `jq` anywhere. Decision: wska carries both the status line and its install; the mod keeps only the resume pane, `set_status` and `/etat`.

Facts (from the official statusline docs):

- The stdin JSON carries `cwd`, `workspace.current_dir`, `model.display_name`, `context_window.used_percentage` (can be null) and `rate_limits.five_hour` / `rate_limits.seven_day`, each `{ used_percentage, resets_at }` with `resets_at` in epoch seconds. Each window may be absent.
- `statusLine.refreshInterval` is in seconds, minimum 1. ANSI is supported. A running script is cancelled when a new update fires.
- A plugin cannot set the main `statusLine`: it must go in the user's settings.

The new split:

- **`wska hud statusline`** draws the segments the band had (same order and thresholds), reads the quotas from stdin, and samples RAM (cgroup v2 working set), CPU (state file `$XDG_RUNTIME_DIR/weebo-hud-cpu`) and disk (`df`, 3 s deadline). It never fails: a missing figure shows a dash.
- **`wska hud install` / `uninstall`** write and remove `statusLine = {type: command, command: "wska hud statusline", refreshInterval: 5}` in `$CLAUDE_CONFIG_DIR/settings.json` or `~/.claude/settings.json`, with a one-time backup `settings.json.weebo-hud-bak`. They never run `jq`.
- **Options** are the `WEEBO_HUD_*` environment variables (`DOSSIER`, `MODELE`, `CONTEXTE`, `CINQH`, `SEPTJ`, `RAM`, `CPU`, `DISQUE`, `RESET5H`, `RESET7J`), set through the `env` block of `settings.json`. Only `paneAuto` stays in the mod's `userConfig`.
- **The mod** (`mods/weebo-hud`) loses the `AbovePrompt` render, the sampler, `metricsAtom`, `metrics.ts` and `format.ts`. Its state holds only `notes`.
- **Installer:** `wska hud install && { claude plugin marketplace add ...; claude plugin install weebo-hud@weebo --scope user; }`. Retiring the old status line is automatic: install replaces the `statusLine` key and names the backup.
- `wska hud` reaches mise-installed machines only after a wska release.

The sections below are kept as history. The band, its metrics and its install notes are superseded by this revision; the resume pane sections still hold.

## The band (`AbovePrompt`) (superseded)

One powerline row, with the segments in this order:

| Segment | Content | Background |
|---|---|---|
| dossier | basename of `$.session.cwd()` | `#005fd7` (xterm 26) |
| modele | `$.session.model()` | `#875fff` (xterm 99) |
| contexte | `ctx 34%` from `usage().context.percent`; `ctx —` when absent | level |
| 5h | `5h 62% 14:30` from `rateLimits[kind=five_hour]` | level |
| 7j | `7j 21% sam 09:00` from `rateLimits[kind=seven_day]` | level |
| ram | `RAM 22.9/32.0G libre` | level |
| cpu | `CPU 12%` | level |
| disque | `disque 120G libre` (one segment per distinct filesystem; with several, each is suffixed with its mount's basename, `/` staying `/`: `disque projects 120G libre`) | level |

- **Text and separators:** the foreground is `#eeeeee`. Segments are joined by U+E0B0 (``): its foreground is the previous segment's background, and its background is the next one's. That is how the old script looks.
- **Level colors:**
  - green `#00af00`, orange `#d7af00`, red `#d70000`.
  - ctx: orange from 50 %, red from 70 %. 5h and 7j: orange from 50 %, red from 80 %. These are the old script's thresholds, kept as they were.
  - CPU (used): orange from 80 %, red from 95 %.
  - For *free* figures (RAM, disk): orange below 20 % free, red below 10 %.
  - Thresholds are hard-coded.
- **Missing data:**
  - A 5h or 7j value with no reading shows `—`. That covers an empty `rateLimits`: before the first API response, with an API-key login, or with a token whose responses carry no limits.
  - A missing `resetsAt` drops the `` part.
  - A metric that cannot be read shows `—` instead of failing the band.
- **Reset time:** in local time. 5h shows `HH:MM`. 7j shows `ddd HH:MM`, with French short day names (`dim lun mar mer jeu ven sam`).
- **Refresh:** metrics are sampled every 5 s, and usage is read at the same tick.
- **Survey:** when `e.props.hasSurvey` is true, the band returns `next(e)` so it does not cover the survey.

### Metrics

- **RAM:** the cgroup v2 limit when there is one. If `memory.max` is not `max`, then free = `memory.max − (memory.current − inactive_file)`, clamped to [0, `memory.max`], with `inactive_file` from `memory.stat` (the working set, as the kubelet counts it; when `memory.stat` cannot be read, free = `memory.max − memory.current`), total = `memory.max`. Otherwise `/proc/meminfo` gives free = `MemAvailable` and total = `MemTotal`.
- **CPU:** the usage between two samples.
  - With a cgroup quota (`cpu.max` is not `max …`): `Δcpu.stat usage_usec / (Δt_usec × quota/period)`.
  - Otherwise, the usage across all cores from `/proc/stat` (`1 − Δidle/Δtotal`, where idle is idle + iowait).
  - The first sample shows `—`.
- **Disk:** `df -kP <cwd> <$HOME>`, deduplicated by filesystem. It shows the available space, and the level is based on available/size.
- **cgroup location:** `/sys/fs/cgroup/` (cgroup v2, unified). On a cgroup v1 host the mod falls back to `/proc`.

## The resume pane (`Pane`, id `weebo-hud`)

Written in French, in three blocks:

- **Objectif** is one line, written by Claude.
- **Plan** mirrors the task list, one line per item: `✓` completed, `▸` in_progress, `·` pending. In-progress items show their `activeForm` (the subject when it has none). Interactive sessions build it with `TaskCreate` (adds a pending item, keyed by the task id from the result) and `TaskUpdate` (status, subject, activeForm by id; `deleted` removes it); a successful `TodoWrite` replaces the whole list. Only successful calls count.
- **Notes** holds free text from Claude: where things stand and the decisions made. A separate **En attente** line holds what Claude is waiting on from the user. It is hidden when empty.

**How it gets content.** Claude writes through the tool `mcp__weebo-hud__set_status`, which has `isDeferred: false`.

- The input is `{ objectif?: string, notes?: string, attente?: string }`. Each field present replaces the stored one, and an empty string clears it. There is no history.
- The tool description is the instruction, and must say: call it when work starts, after each step completed, and whenever you wait on the user; main conversation only; write in the conversation's language.
- The tool and `/etat` are registered only when `session.start` reports `isInteractive`; headless runs (`-p`, SDK) keep the band and the sampler.
- **Main conversation only:** `set_status`, `TodoWrite` and `Task*` calls carrying an `agentId` (a subagent's or a teammate's loop) are ignored; `set_status` still answers `ok`.
- `CLAUDE.md` gets one line pointing at the tool. In this repo it goes in the Skills section, and it is then carried to weebo-base's `CLAUDE.md`.

**Persistence.** The data lives in `$.store` under the key `session:<sessionId>`, with the value `{ objectif, notes, attente, todos, updatedAt }`.

- It is reloaded at `session.start`, which also covers `--resume` (the same session id).
- `/clear` and an in-REPL `/resume` change the session id without a `session.start`. The mod tracks which session its notes belong to: every save checks `$.session.id()` and, on a change, reloads from `session:<newId>` before merging, so one session's notes are never written under another's key. The pane draws the new session's stored notes until then, and `session.end` resets the tracking.
- Entries whose `updatedAt` is more than 30 days old are deleted at `session.start`.

**Opening.**

- If the `paneAuto` option is on, the pane opens (`$.ui.open`, no focus) as soon as the session has any content: at start when content was stored, otherwise on the first `set_status` or plan update. It auto-opens at most once per session: a pane the user closed stays closed until `/etat`.
- `/etat` toggles the pane open or closed.

## Configuration (`userConfig`, all booleans, default `true`) (band options superseded: only `paneAuto` remains)

`dossier`, `modele`, `contexte`, `cinqH`, `septJ`, `ram`, `cpu`, `disque`, `reset5h`, `reset7j`, `paneAuto`.

Changing a value in `/config` reloads the module. Everything else is hard-coded.

## Install and retire (superseded: see the Revision)

- `vendor.toml` gets `[installers.weebo-hud]` with scope `user`, since the band replaces a user-wide status line. The item joins the `base` profile.
- After the user validates the mod, Claude gives them (it does not run them) the commands that remove `statusLine` from `~/.claude/settings.json` and delete `~/.claude/statusline.sh`.

## Out of scope (V1)

Configurable thresholds, colors, order and interval; a styled fallback outside the terminal; note history; a reminder toast when Claude forgets the tool; showing cost or git branch.
