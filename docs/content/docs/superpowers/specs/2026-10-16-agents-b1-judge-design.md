---
title: "Agents B1 — judge and the first two reviewers"
---

- Date: 2026-10-16
- Status: proposed
- Sub-project: B1, the first slice of sub-project B (agents) from
  `2026-10-15-skills-spine-design.md`

## Intent

Headless review gates that any project can run, built so that judgement is
cheap and repeatable wherever it can be and an LLM is used only where
reasoning is needed.

B1 delivers the framework every later reviewer reuses, plus two consumers
that exercise it from opposite ends:

- **`rfc-ready`** — is an RFC ready for sign-off, how far can its claims be
  trusted, and how sure is that verdict. No LLM: deterministic checks plus
  Jev.
- **`doc-review`** — is the change properly documented. An LLM agent finds
  candidate gaps; Jev filters out the ones that do not hold up.

Success: in any project with the `review` profile installed,
`judge rfc-ready` and `judge doc-review` each print a JSON report and exit
with a code a hook or CI job can act on, and the whole test suite passes
without a TypeSafe API key.

## Scope

In scope:

- The `judge` Python package and CLI: Jev backend, question files, report
  and exit-code contract, finding filter.
- `rfc-ready` and `doc-review`.
- `wska` changes: `scope = "any"` for installers, `WSKA_REPO` in installer
  environments.
- Items, profile, requirements and tasks wiring.

Out of scope, each its own later spec:

- B2 — doc generation (the first agent that writes files).
- B3 — code review.
- B4 — security review (OWASP, post-mortem-driven checks). Must settle the
  data-egress question below before sending code to Jev.
- Wiring any gate into git hooks or CI (GitHub and Forgejo Actions). The
  exit-code contract is what those will call; nothing here assumes either.
- A non-Jev judge backend (Haiku, local logprobs). The `Judge` protocol
  leaves room for one.

## Decisions

### Jev is a function, not an agent

Jev (TypeSafe's System One model) takes a state and typed questions —
Choice, Score, Noul — and returns calibrated probabilities. It generates no
text. TypeSafe's own docs position it as something code calls, and their
agent skill (`typesafe-ai/skills`) only teaches an agent to write such code.
So Jev sits behind a function the pipelines call with **fixed, versioned
question files**. An agent improvising its own Jev questions per run would
throw away the one property that makes Jev worth using: the same rubric,
judged the same way, every time.

### LLM only where reasoning is needed

`rfc-ready` never calls Claude: heading presence, status, open-question
count and symbol existence are code; whether a section has substance is a
Jev question. `doc-review` needs an LLM to find what is undocumented, and
uses Jev only to filter what it found. Rejected: everything-is-a-Claude-agent
(an LLM paying to grep, and a drifting rubric).

### Not in `wska`

The spine spec keeps `wska` to *where files go*, never *what they mean*. A
judge is all meaning, so it is its own package. `wska` gains only two small
mechanical changes (see `wska` changes).

### The SDK owns the wire format

`typesafe-sdk` (PyPI, MIT, `typesafe-ai/typesafe-sdk-python`) is used rather
than raw HTTP. The request shape is only known from docs and third-party
write-ups; the SDK is what TypeSafe keeps correct. Tests fake the client at
the SDK boundary, so no test depends on a key or the network.

### Headless first, interactive as a side effect

The primary consumer is a hook or CI job, so every command emits a JSON
report and a meaningful exit code. `agents/doc-review.md` is an ordinary
Claude Code agent file, so it also works as an interactive subagent.

## Layout

```
agents/doc-review.md               Claude agent, read-only tools
judge/
  pyproject.toml                   console script `judge`; deps: typesafe-sdk (pinned), pyyaml
  src/judge/
    backend.py                     Judge protocol; JevBackend (SDK), FakeBackend (tests)
    questions.py                   question YAML -> SDK Choice / Score / Noul
    report.py                      report shape, verdict, exit code
    rfc.py                         RFC parsing: header table, sections, symbols
    rfc_ready.py                   rfc-ready pipeline and scoring
    excerpt.py                     re-read cited lines from disk
    filter.py                      finding filter
    doc_review.py                  runs claude -p, then the filter
    cli.py                         argument parsing, dispatch
  questions/rfc-ready.yaml         rubric, weights, thresholds
  questions/finding-filter.yaml    filter question, threshold
  schemas/findings.schema.json     the shared finding shape
  tests/                           pytest, fixtures/
.tasks/agents.yaml                 agent:* tasks
```

## The judge backend

```python
class Judge(Protocol):
    model: str | None   # e.g. "jev-1.13.0", from the response
    def ask(self, state, questions: dict[str, Question]) -> dict[str, Answer]: ...
```

- `JevBackend` wraps `TypeSafeClient.system_one`, model `jev-latest`, and
  records the resolved model version from the response into the report.
  Retries on 429/529 are the SDK's.
- `FakeBackend` returns answers loaded from fixture files built from SDK
  response types.
- Backend selection: `TYPESAFE_API_KEY` set → `JevBackend`; unset → none.

### Without a key, or without Jev

| Condition | Behaviour | Exit |
| --- | --- | --- |
| No `TYPESAFE_API_KEY` | deterministic checks still run; Jev questions skipped; verdict `unjudged` | 3 |
| Jev unreachable after SDK retries (network, 429, 529) | same as above, reason recorded | 3 |
| 400 / 401 / 422 from Jev | error: a malformed rubric or a bad key is a bug to fix, not a judgement | 2 |

`unjudged` is distinct from pass so no gate can mistake "could not judge"
for "ready". A deterministic failure (a missing mandatory heading) still
yields `fail` / exit 1 with no key — a missing block needs no judge.

## Question files

YAML, one file per pipeline, loaded and validated at startup. Each question
has an id, a type (`choice` | `score` | `noul`), `instructions`, and
`criteria` (map for choice, ordered list for score). Pipeline-specific keys
(`weights`, `thresholds`) sit beside the questions they apply to. A question
file that does not build into SDK question objects is exit 2 at startup,
and is caught earlier by the contract test.

## Report and exit-code contract

Every command prints one JSON document to stdout (`--format text` for
people), shared top level:

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

- `verdict`: `pass` | `fail` | `unjudged`. Over several items, the worst
  wins: `fail` > `unjudged` > `pass`.
- Exit codes: **0** pass, **1** fail, **2** usage or internal error,
  **3** unjudged.
- Every finding carries `file`, `line` (when known), `message`, `source`
  (`check:<name>` or `jev:<question-id>`) and `probability` (null for
  deterministic checks).

## `rfc-ready`

```
judge rfc-ready [FILE...] [--root DIR]
```

With no file, scans `docs/rfc/*.md` (relative to `--root`, default the git
root) and skips `Implemented` and `Rejected`; a named file is always judged.
The rules come from the `batlehub-rfc` skill (its §4 mandatory blocks and §7
`review` order); this pipeline mechanises them and does not restate them.

### Deterministic checks

| Check | Fails the RFC when |
| --- | --- |
| `heading` | any of §6.P, §7, §8, §9, §10, §11 is missing — Jev is then not called for that RFC |
| `open-questions` | *Still open* under §15 has one or more items |
| `template-comment` | an HTML comment from the template remains (each is a finding) |
| `symbols` | never fails; yields the symbol existence rate |

Symbols: backticked tokens that look like code — contain `::`, `/` with a
file extension, `()`, or are `snake_case` / `CamelCase` identifiers of 3+
characters — checked with `git grep -F -q` in the target repo. Tokens inside
fenced code blocks are ignored (examples, not claims).

### Jev questions

One request per state, all questions for that state batched.

| State | Question | Type |
| --- | --- | --- |
| each mandatory block | `block-quality` | Score: `placeholder` < `bare none` < `reasoned none` < `substantive` |
| §7 + §14 | `security-enforced`: every security claim names the code that enforces it | Noul |
| §7 + §14 | `negative-tests`: every threat row has a negative test in §14 | Noul |
| §4.1 + §10.2 | `safe-defaults`: new defaults are the safe side and controls fail closed, or the text argues otherwise | Noul |
| whole RFC, code blocks stripped | `probed-numbers`: numbers come from stated probes or measurements, not estimates | Noul |

### Verdict and the two numbers

- **Ready** (`pass`) iff: every heading present, every block's
  `block-quality` ≥ `reasoned none`, no open questions, and
  `security-enforced`, `negative-tests`, `safe-defaults` each ≥ the
  threshold (default 0.5, in the YAML).
- **Claim trust** (0–1): weighted mean of the symbol existence rate and the
  `security-enforced`, `negative-tests`, `safe-defaults`, `probed-numbers`
  probabilities. Weights in `rfc-ready.yaml`. Informational; does not gate.
- **Verdict confidence** (0–1): the minimum certainty among answers that
  decided the verdict — Score `confidence` as returned, Noul
  `2·|p − 0.5|`. Conservative on purpose: one shaky deciding answer makes
  the verdict shaky.

Per-RFC item: `file`, `status`, `verdict`, `claim_trust`,
`verdict_confidence`, `findings`.

## `doc-review`

```
judge doc-review [--base REF | --all] [--budget USD]
```

### The agent

`agents/doc-review.md`: model `sonnet`; tools Read, Grep, Glob, and Bash
limited to `git diff`, `git log`, `git ls-files`. It reads the target
repo's `CLAUDE.md` / `AGENTS.md` for house rules rather than hardcoding
them, and looks for:

- behaviour added or changed in the diff without a doc update (public
  functions, CLI flags, config keys, API routes, tasks);
- docs contradicting the code (wrong flag names, removed options, stale
  examples);
- house-rule gaps the repo states (a task without `desc`, an API route
  without its OpenAPI annotation, no `docs/` folder).

### The run

`judge doc-review` invokes

```
claude -p --agent doc-review --output-format json \
  --json-schema <findings.schema.json> \
  --max-budget-usd <budget, default 1.00> "<scope prompt>"
```

in the target repo's root. The schema path is resolved from the installed
package (`importlib.resources`), never from the working directory, since
the target repo has no `judge/`. The scope prompt names `git diff <base>...HEAD` (default base
`origin/main`) or the whole repo with `--all`. A non-zero `claude` exit or
output failing the schema is exit 2.

Finding shape (shared by every later reviewer):

```json
{"file": "docs/x.md", "line": 12, "kind": "missing|stale|wrong|unclear",
 "claim": "…", "code_ref": {"file": "src/y.rs", "line": 40}, "suggestion": "…"}
```

### The filter

`judge filter --in findings.json` (also used internally):

1. Re-read ±10 lines around `file:line` and `code_ref` from disk. The
   agent's own quotes are never used. A finding citing a missing file or a
   line past EOF is dropped as `fabricated-ref` without asking Jev.
2. One Noul per surviving finding, state = finding + real excerpts:
   `actionable` — "the claim is true of these excerpts and a maintainer
   would change something because of it". All in one batched request.
3. Keep at or above the threshold (default 0.6, in
   `finding-filter.yaml`). Dropped findings stay in the report under
   `dropped` with their probability, so the filter itself can be audited.

Verdict: `fail` if any finding is kept, else `pass`. Without a key: all
non-fabricated findings reported unfiltered, verdict `unjudged`, exit 3.

## `wska` changes

1. **`scope = "any"`** for installers: runs at whichever scope is invoked.
   `judge` is a machine-wide `uv tool` install; declared `user` it would be
   skipped by the default `--scope project`, leaving `doc-review` linked
   but unable to run.
2. **`WSKA_REPO`** set in every installer's environment to the repo root,
   so an installer can reference this repo's files. One `.env()` on the
   installer `Command`, one test.

## Wiring

`vendor.toml`:

```toml
[installers.judge]
command = 'uv tool install --force --editable "$WSKA_REPO/judge"'
scope   = "any"

[requires]
judge      = ["uv"]
doc-review = ["claude", "judge"]
```

`--editable` mirrors the symlink model: edits in this repo are live
everywhere without reinstalling.

`profiles.toml`: new `review = ["judge", "doc-review"]` including `base`;
`planning` gains `judge` (so `rfc-ready` comes with `batlehub-rfc`).

`.tasks/agents.yaml`, added to `Taskfile.yaml`'s `includes` as `agent`:

- `agent:rfc-ready` — `judge rfc-ready {{.FILE}}`
- `agent:doc-review` — `judge doc-review` with `BASE` / `ALL`
- `agent:test` — `uv run --project judge pytest`

`task audit` gains a Python dependency audit of `judge/` (`pip-audit` via
`uvx`, on the exported lockfile). `task lint` gains `uvx ruff check judge`
and `uvx ruff format --check judge`. `uv` is already in `mise.toml`.

Docs: `docs/judge.md` (usage, report contract, exit codes, question-file
format); readme item table and profile table updated.

## Testing

No test needs a key, the network, or an LLM.

- Unit: header-table and section parsing, heading detection, open-question
  count, symbol extraction (including code-fence exclusion), trust and
  confidence maths, verdict aggregation, exit-code mapping, excerpt
  re-reading including the fabricated-ref cases.
- Pipeline, with `FakeBackend`: a ready RFC; one missing §9 (fails, Jev not
  called); filler in §7 (fails on `block-quality`); a borderline Noul giving
  low verdict confidence; no key (`unjudged`, exit 3, deterministic
  findings still present).
- Contract: every question YAML builds into SDK question objects.
- `doc-review` end to end with a stub `claude` on `PATH` returning canned
  findings, one of them citing a non-existent line.
- `wska`: `scope = "any"` runs at both scopes; `WSKA_REPO` reaches the
  installer.
- Manual, by the author with a key: first live `rfc-ready` and
  `doc-review` runs; the raw responses are saved as fixtures so the fake
  matches reality.

## Risks

- **Data egress.** Jev is US-hosted, and its retention and training policy
  is undocumented. RFCs and doc excerpts go to it. Acceptable for B1;
  B4 (security review) must not send code or findings until that is
  answered.
- **Vendor claims are unverified.** Speed, cost and calibration are
  TypeSafe's own numbers. The first live runs are the check; thresholds
  live in YAML so they can move without code changes.
- **Early-access API.** `jev-latest` can change under us; the resolved
  model version is in every report so a verdict shift can be traced.
- **Context length** is not documented. One request per block keeps
  `rfc-ready` states small; an oversized state surfaces as a 400 (exit 2),
  not a silent truncation.
- **Unpublished rate limits.** SDK retries, then `unjudged` — never a
  false pass.
