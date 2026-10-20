---
title: "judge — headless review gates"
---

`judge` runs review gates that a hook or CI job can act on. Each command
prints one JSON report on stdout (`--format text` for people) and exits:

| Exit | Meaning |
| --- | --- |
| 0 | pass |
| 1 | fail |
| 2 | usage or internal error: bad flag, bad rubric, bad key, malformed agent output, crash |
| 3 | unjudged: a judgement was needed and Jev could not give one |

`unjudged` is never a pass, so no gate can mistake "could not judge" for
"ready". Without `TYPESAFE_API_KEY`, the deterministic checks still run, and a
deterministic failure is still exit 1.

Judgement uses Jev (TypeSafe's System One model) through `typesafe-sdk`,
always with the fixed question files below and never with questions made up
per run. The backend is chosen once: `TYPESAFE_API_KEY` set means Jev, unset
means no judge. Rate limits, 5xx (529 included) and network failures are
retried by the SDK, then reported as `unjudged`. A 400, 401, 403, 404 or 422
is exit 2: a malformed rubric or a bad key is a bug to fix.

## Install

```bash
task skills:install PROFILE=review    # links the doc-review agent, installs judge
```

The `judge` installer runs `uv tool install --force --editable
"$WSKA_REPO/judge"` at either scope, so edits in this repo are live everywhere.
The `planning` profile installs `judge` alone (rfc-ready needs no agent).

Tasks in this repo: `task agent:rfc-ready [FILE=…]`, `task agent:doc-review
[BASE=…] [ALL=1]`, `task agent:test`.

## `judge rfc-ready [FILE...] [--root DIR]`

Is an RFC (batlehub-rfc form) ready for sign-off, how far can its claims be
trusted, and how sure is the verdict. With no file, it judges every
`docs/rfc/*.md` under `--root` (default: the git root) except those whose
`Status` starts with `Implemented`, `Rejected` or `Superseded` (bold and
trailing text allowed), and files with no `Status` row at all (an index, a
plan). A file named on the command line is always judged. Mandatory blocks are
found by heading level and title, not number, since the template closes up
numbering when optional sections are deleted: a §8 titled anything but
"Supply chain" counts as missing.

Deterministic checks, which need no key:

| Check | Fails the RFC when |
| --- | --- |
| `heading` | any of §6.P, §7, §8, §9, §10, §11 is missing. Jev is then not called for that RFC |
| `open-questions` | the *Still open* heading, wherever it sits, has one or more items |
| `template-comment` | an HTML comment remains (one finding each) |
| `symbols` | never fails; gives the share of backticked code-like tokens (`a::b`, `f()`, `path/file.ext`, `snake_case`, `CamelCase`) found in the repo's tracked non-markdown files |

Fenced code is ignored by every check: examples are not claims.

Jev questions, one request per state:

| State | Question | Gates |
| --- | --- | --- |
| each mandatory block | `block-quality`: placeholder < bare none < reasoned none < substantive | P(≥ reasoned none) ≥ threshold |
| §7 + §14 | `security-enforced`, `negative-tests` | p ≥ threshold |
| §4.1 + §10.2 | `safe-defaults` | p ≥ threshold |
| whole RFC, code stripped | `probed-numbers` | no (trust only) |

Each item has these fields:

- `file`, `status`, `verdict`, `findings`.
- `claim_trust` (0–1, informational): weighted mean of the symbol rate and the
  four Noul probabilities that are available.
- `verdict_confidence` (0–1): the least certain answer that decided the
  verdict. For a Score that is its `confidence`; for a Noul it is
  `2·|p − 0.5|`. It is 1.0 for a deterministic failure and null when unjudged.

## `judge doc-review [--base REF | --all] [--budget USD]`

Is the change properly documented? It runs, in the target repo's root:

```
claude -p --agent doc-review --output-format json --json-schema '<schema>' \
  --max-budget-usd <budget> --allowedTools "Read Grep Glob Bash(git diff:*) …"
```

The scope prompt is sent on stdin. It names `git diff <base>...HEAD` (default
base `origin/main`, default budget 1.00 USD), or the whole repo with `--all`.
`--base` is checked with `git rev-parse` before `claude` starts, so a typo
costs nothing. The agent (`agents/doc-review/doc-review.md`, read-only)
follows the repo's own `CLAUDE.md`/`AGENTS.md`. Its findings then go through
the filter. Verdict: `fail` if any finding is kept.

## `judge filter --in findings.json`

The filter on its own, also used by doc-review:

1. Each finding's `file:line` and `code_ref` are re-read from disk, ±10 lines.
   Only files git tracks inside the repo are read. A finding citing anything
   else, or a line past the end of the file, is dropped as `fabricated-ref`
   without asking Jev.
2. One Noul per surviving finding, `actionable`, all in one request.
3. Findings at or above `threshold` are kept. Dropped findings stay under
   `dropped` with `dropped_because` (`fabricated-ref` | `below-threshold`) and
   their probability.

Without a key, surviving findings are reported unfiltered and the verdict is
`unjudged`. A run with nothing to judge is a pass.

## Report

```json
{
  "tool": "rfc-ready",
  "report_version": 1,
  "verdict": "pass",
  "judge_model": "jev-1.13.0",
  "unjudged_reason": null,
  "items": []
}
```

- `verdict`: `pass` | `fail` | `unjudged`. Over several items the worst wins:
  `fail` > `unjudged` > `pass`.
- `judge_model`: the model Jev resolved `jev-latest` to, so a verdict shift can
  be traced to a model change.
- Every finding carries `file`, `line` (or null), `message`, `source` and
  `probability` (null for deterministic checks). `source` is one of:
  - `check:<name>`, a deterministic check;
  - `jev:<question-id>`, a Jev answer;
  - `agent`, an agent finding no judge has filtered.
- doc-review and filter findings add `kind` (`missing` | `stale` | `wrong` |
  `unclear`), `code_ref` and `suggestion`. Their report adds `dropped`.

The finding shape the agent must produce is
`judge/src/judge/schemas/findings.schema.json`; every later reviewer shares it.

## Question files

`judge/src/judge/questions/<pipeline>.yaml`:

```yaml
version: 1
questions:
  <id>:
    type: score            # choice | score | noul
    instructions: "…"
    criteria: ["lowest", "…", "highest"]   # map for choice; true/false map for noul
thresholds: {…}            # pipeline keys sit beside the questions
```

Thresholds and weights are moved here, never in code. A file that does not
build into SDK question objects is exit 2 at startup, and `task agent:test`
catches it first.

## Data egress

RFC text and doc/code excerpts are sent to Jev (TypeSafe, US-hosted; retention
and training policy undocumented). Untracked and git-ignored files are never
read. B4 (security review) must settle this before sending code.
