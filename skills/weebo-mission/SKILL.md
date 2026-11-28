---
name: weebo-mission
description: >
  Writing or changing a weebo-agent fleet maintenance mission (RFC 0001 of
  batleforc/weebo-update) — the `missions/<name>/mission.yaml` schema and the
  rules `task fleet:check` enforces, the prompt rules (no untrusted text, G3),
  the output modes, the deterministic check script, the outcome and exit-code
  table of `run-mission`, and running a mission locally with `task mission:run`.
  Use when adding or editing a directory under `missions/`, a mission prompt,
  a `fleet.yaml` entry, or when a mission run ends in an unexpected outcome.
---

# Weebo mission

A mission is a directory `missions/<name>/` of
[weebo-update](https://github.com/batleforc/weebo-update), **baked into the runner image**:
a target repo can never define or change its own mission. The runner (`run-mission`) runs it
in three containers of one pod, each with its own credentials:

| Container | Does | Holds |
| --- | --- | --- |
| `prepare` | validates inputs, clones, runs `precheck` and `check` | no token |
| `agent` | calls the model through `weebo-llm-proxy`, only if needed | the proxy token (audience `weebo-llm-proxy`), never a GitHub token |
| `publish` | writes the issue, branch or PR with a repo-scoped GitHub App token | the OpenBao token; the model has exited |

The source of truth is in weebo-update: `docs/mission-authoring.md` (schema),
`runner/run-mission` (behaviour), `scripts/fleet-check.jq` (rules). When this skill and the
code disagree, the code wins.

## 1. `mission.yaml`

```yaml
name: health                        # = directory name
trigger:
  schedule: "0 5 * * 1"             # 5-field cron, read by dispatch (Europe/Paris)
  events: [manual]                  # manual | ci-failed | push:<b> | label:<n> | comment:/agent <w>
  only_authors: ["renovate[bot]"]   # required, non-empty, with ci-failed
precheck: task ci:local             # exit 0 → outcome noop, no model call
check: check.sh                     # deterministic script in the mission directory, JSON on stdout
prompt: prompt.md                   # present = the mission may call the model
output: issue                       # report (default) | issue | push-branch | branch-pr
limits: { max_turns: 3, per_run_usd: 0.1, timeout: 15m }
tools: { allow: ["Read"], deny: [] }
scope: ["src/**", "Cargo.lock"]     # globs a writing mission may change
```

Absent means the closed side: `output: report`, `tools: Read` only, `limits` of 10 turns and
$0.5 on a read-only mission, no PR eligible without `only_authors`.

## 2. What `task fleet:check` refuses

- an unknown key (a typo never falls back to a default);
- a writing output (`push-branch`, `branch-pr`) without `limits`;
- in `tools.allow`: `WebFetch`, `WebSearch`, `Bash`, `Bash(*)`, `Bash(git:*)`, `Bash(git push…)`,
  `Bash(gh…)`, `Bash(curl…)`;
- a `scope` glob that reaches a protected path (`runner/protected-paths.txt`), or has no literal
  prefix (`**/*.md` reaches `CLAUDE.md`);
- `ci-failed` without `only_authors`; a missing `prompt` or `check` file;
- in `fleet.yaml`: a private repo with a model mission and no explicit `allow_llm`, a mission that
  does not exist under `missions/`.

Run it after every change. It is also in CI.

## 3. Prompt rules

- **No untrusted text in the prompt** (RFC 0001 G3). The prompt is the mission's `prompt.md`
  plus data the image produced (the check's JSON, a CI log the runner fetched). Never a PR
  body, an issue, a comment or a commit message: anyone can write those.
- Data the runner appends goes in a fenced block labelled `untrusted: <what>`; the prompt
  tells the model it is data, not instructions.
- Say the output shape exactly (lines, format, length). The runner reads `result` from
  `claude -p --output-format json`; it does not parse prose.
- The model runs with `--bare` (api credential) or `--setting-sources user` with the repo's
  `CLAUDE.md` set aside (subscription): the target repo's instructions and hooks never apply.
- Keep the turn and cost limits tight: the proxy enforces them per run, day and month, and a
  refusal is an outcome (`budget`, `window`), not a crash.

## 4. Deterministic first

Anything a script can decide, a script decides: a `precheck` that ends the run when green, a
`check` that computes the result. The model is for what needs judgment and only runs when the
check says so (health calls it only when the score moved, and only on `allow_llm` repos).

## 5. Outcomes and exit codes

`run-mission` writes `result.json` with one outcome; `publish` always runs.

| Outcome | Exit | Means |
| --- | --- | --- |
| `pushed`, `noop`, `reported`, `skipped`, `queued` | 0 | normal end |
| `stale`, `error` | 1 | the branch moved, or something broke (clone, tool, empty result) |
| `refused-pre` | 2 | invalid input, repo not in `fleet.yaml`, mission not enabled, `allow_llm` |
| `refused-post`, `gave-up` | 3 | the post-checks rejected the change, or the model gave up |
| `budget`, `window` | 4 | the proxy refused on a cap or outside the time window |
| `egress` | 5 | a forbidden host was reachable, or the proxy was not |

## 6. Run it locally

```bash
task fleet:check
task mission:image                                    # runner image with podman
task mission:run MISSION=health REPO=github:batleforc/weebo-base UNSAFE_EGRESS=1
```

`UNSAFE_EGRESS=1` skips the egress probe (no network policy on a laptop) and says so in the
log. Without `TOKEN_FILE` there is no model call; with one, `task proxy:local` serves the
proxy. Tests: `tests/runner/*.bats` stub git, curl, claude and task — add a case there for any
new outcome path.

Related: `weebo-health` (the health scorecard), `weebo-taskfile-mise` (`ci:local`, `audit`).
