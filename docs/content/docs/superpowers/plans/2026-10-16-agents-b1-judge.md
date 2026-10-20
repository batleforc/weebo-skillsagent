---
title: "Agents B1 — judge Implementation Plan"
---

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `judge` Python CLI with two headless review gates. `judge rfc-ready` uses deterministic checks plus Jev. `judge doc-review` runs a Claude agent and filters its findings with Jev. Each command prints a JSON report and exits 0, 1, 2 or 3. wska learns `scope = "any"` installers and passes them `WSKA_REPO`.

**Architecture:** `judge/` is a standalone uv project with a console script. All judgement goes through one `Judge` protocol: `JevBackend` wraps `typesafe-sdk`, and `FakeBackend` replays wire-shaped JSON fixtures, so no test needs a key. Rubrics are fixed YAML files shipped inside the package. Pipelines return plain-dict reports, and `cli.py` maps a report to an exit code. wska's changes are mechanical: a new `InstallerScope` enum and an `.env()` call on the installer command.

**Tech Stack:** Python ≥3.12, uv, hatchling, `typesafe-sdk==0.7.5`, PyYAML, pytest, ruff; Rust (wska); Claude Code CLI 2.1.x; Taskfile.

**Spec:** `docs/superpowers/specs/2026-10-16-agents-b1-judge-design.md`

## Global Constraints

- Exit codes: **0** pass, **1** fail, **2** usage or internal error, **3** unjudged. A crash is 2, never 1.
- `verdict`: `pass` | `fail` | `unjudged`; over several items the worst wins: `fail` > `unjudged` > `pass`.
- Report top level: `tool`, `report_version: 1`, `verdict`, `judge_model`, `unjudged_reason`, `items`.
- Every finding carries `file`, `line` (or null), `message`, `source`, `probability` (null for deterministic checks).
- `typesafe-sdk` is pinned (`==0.7.5`); model `jev-latest`; the resolved model (`response.model`) goes in the report.
- Backend: `TYPESAFE_API_KEY` set (non-blank) → `JevBackend`; unset → no backend.
- 400/401/403/404/422 or a client-side SDK error → exit 2. Rate limit (429), 5xx (including 529), connection error or timeout after SDK retries → `unjudged`.
- No test needs a key, the network or an LLM.
- Mandatory RFC blocks: §6.P, §7, §8, §9, §10, §11.
- Defaults: Noul gate threshold 0.5 (`rfc-ready.yaml`), filter threshold 0.6 (`finding-filter.yaml`), doc-review budget 1.00 USD, base `origin/main`.
- Question files and the schema are read with `importlib.resources`, never from the working directory.
- The agent's own quotes are never sent to Jev. Excerpts are re-read from disk, ±10 lines.
- Every Taskfile task has a `desc`. Shared tasks go in `.tasks/` (not `.task/`).
- **No commits by the agent.** The user signs and signs off every commit. Each task ends by staging the files and writing the message to `.task/commit-msg.txt` (gitignored). The user then runs `git commit -s -S -F .task/commit-msg.txt`. Every message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Prefix shell commands with `rtk` where the repo's CLAUDE.md says to (git, cargo, pytest via `rtk test`).

### Deviations from the spec's layout, decided while planning

1. **`agents/doc-review/doc-review.md`, not `agents/doc-review.md`.** wska's `scan` only treats directories as items (`crates/wska/src/item.rs:459`), and the unlink pass matches link names to item names. A `.md` file item would need new code in scan, plan, status and doctor. Claude Code finds agents in subdirectories of `.claude/agents/`. This was verified on 2026-10-16: `claude -p --agent doc-probe` ran `.claude/agents/doc-probe/doc-probe.md`.
2. **`questions/` and `schemas/` live under `src/judge/`**, so they ship in the wheel and `importlib.resources.files("judge")` finds them in an installed tool.
3. **`--json-schema` takes the schema text, not a path** (`claude --help`). `doc_review.py` reads the packaged file and passes its contents.
4. **Structured output is read from `structured_output`, with `result` as the fallback.** On 2026-10-16 a `--json-schema` run returned `structured_output: null` and the answer in `result`.
5. **`--allowedTools` is added to the `claude -p` call** so the agent's `git diff/log/ls-files` calls are pre-approved. In `-p` mode any other Bash call has no one to approve it, so it is denied.
6. **Unfiltered findings get `source: "agent"`.** This covers findings that were never judged (no key, or Jev unreachable) and fabricated ones. Neither `check:` nor `jev:` describes them.
7. **The excerpt reader only reads git-tracked files inside the root.** A finding citing an untracked or ignored file is dropped as `fabricated-ref`. This stops a `.env` from ever being sent to Jev (see the spec's data-egress risk).
8. **Symbol lookup excludes `*.md`.** Otherwise every token would "exist" in the RFC that mentions it.

## Review Focus

1. **A token that appears only in docs.** A backticked name mentioned only in the RFC itself, or in another `.md` file, must count as *missing*. Pinned in Task 4 (`test_symbol_only_in_markdown_is_missing`).
2. **The agent cites a path it should never read:** `.env` (ignored), `../outside`, `/etc/passwd`, `.git/config`. The finding must be dropped as `fabricated-ref` and its content must never reach Jev. Pinned in Task 6 (`test_excerpt_refuses_untracked_and_outside_paths`, `test_filter_never_sends_untracked_content`).
3. **A bug in judge, such as a KeyError in a pipeline.** It must exit 2 and never 1, so a gate cannot read a crash as "not ready". Pinned in Task 7 (`test_unexpected_exception_is_exit_2`).
4. **`--base` names a ref that does not exist, or starts with `-`.** It must exit 2 before `claude` is spawned, so no budget is spent. Pinned in Task 7 (`test_bad_base_fails_before_spawning_claude`).
5. **Markdown inside fenced code:** a `## 9. Data` heading in an example block, or a `1. item` inside a fence under *Still open*. These must not count as a heading or as an open question. Pinned in Task 4 (`test_fenced_heading_and_item_are_ignored`).

---

## File map

| File | Responsibility |
| --- | --- |
| `crates/wska/src/config.rs` | `InstallerScope { Project, User, Any }` + `admits` |
| `crates/wska/src/{item,plan,apply,doctor,status}.rs` | use `InstallerScope`; `RunInstaller.repo_root`; `WSKA_REPO` env |
| `judge/pyproject.toml`, `judge/uv.lock` | package, console script `judge` |
| `judge/src/judge/report.py` | finding/report constructors, worst-wins, exit code, text format |
| `judge/src/judge/backend.py` | `Judge`, `JevBackend`, `FakeBackend`, `JudgeError`, `Unreachable`, `select_backend` |
| `judge/src/judge/questions.py` | YAML → SDK `Choice`/`Score`/`Noul` |
| `judge/src/judge/questions/*.yaml` | rubrics, thresholds, weights |
| `judge/src/judge/rfc.py` | RFC parsing and deterministic checks |
| `judge/src/judge/rfc_ready.py` | rfc-ready pipeline and scoring |
| `judge/src/judge/excerpt.py` | re-read cited lines from disk (tracked files only) |
| `judge/src/judge/filter.py` | finding validation and the Jev filter |
| `judge/src/judge/schemas/findings.schema.json` | the shared finding shape |
| `judge/src/judge/doc_review.py` | runs `claude -p`, then the filter |
| `judge/src/judge/cli.py` | argparse, dispatch, exit codes |
| `judge/tests/…` | pytest + fixtures |
| `agents/doc-review/doc-review.md` | the Claude agent |
| `vendor.toml`, `profiles.toml` | installer, requires, `review` profile |
| `.tasks/agents.yaml`, `Taskfile.yaml` | `agent:*` tasks, lint, audit |
| `docs/judge.md`, `docs/wska.md`, `readme.md` | docs |

---

### Task 1: wska — `scope = "any"` and `WSKA_REPO`

**Files:**
- Modify: `crates/wska/src/config.rs` (Installer struct, around line 83; test at line 175)
- Modify: `crates/wska/src/item.rs` (import line 1, `Item::Installer`, tests)
- Modify: `crates/wska/src/plan.rs` (`Action::RunInstaller`, installer branch around line 100, tests)
- Modify: `crates/wska/src/apply.rs` (RunInstaller arm around line 83, tests at 254/270)
- Modify: `crates/wska/src/doctor.rs` (test import line 295, test at 385)
- Modify: `crates/wska/src/status.rs` (test import line 56, test at 114)

**Interfaces:**
- Produces: `wska::config::InstallerScope` (`Project | User | Any`, serde lowercase), `InstallerScope::admits(self, Scope) -> bool`; `Action::RunInstaller { name, command, cwd, repo_root }`; installers run with env `WSKA_REPO=<repo_root>`.

Note: another session has uncommitted edits in `main.rs`, `target.rs`, `Taskfile.yaml`, `docs/wska.md` and `readme.md`, and `main.rs` there reads `WSKA_REPO` as the items-repo root. That is the same meaning, so nothing conflicts. Do not touch `main.rs`. Run `rtk git status` first and leave those files alone in this task.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `crates/wska/src/config.rs`:

```rust
    #[test]
    fn parses_an_any_scope_installer() {
        let (_d, p) = tmp("[installers.judge]\ncommand = \"true\"\nscope = \"any\"\n");
        let got = load_vendor(&p).unwrap();
        assert_eq!(got.installers["judge"].scope, InstallerScope::Any);
    }

    #[test]
    fn any_admits_both_scopes_and_the_others_only_their_own() {
        use crate::cli::Scope;
        assert!(InstallerScope::Any.admits(Scope::Project));
        assert!(InstallerScope::Any.admits(Scope::User));
        assert!(InstallerScope::Project.admits(Scope::Project));
        assert!(!InstallerScope::Project.admits(Scope::User));
        assert!(!InstallerScope::User.admits(Scope::Project));
    }
```

Append to the `tests` module in `crates/wska/src/plan.rs`:

```rust
    #[test]
    fn an_any_scope_installer_runs_at_both_scopes() {
        let f = fx(&[]);
        let mut map = BTreeMap::new();
        map.insert(
            "judge".to_string(),
            Item::Installer {
                name: "judge".into(),
                command: "true".into(),
                scope: InstallerScope::Any,
            },
        );
        for scope in [Scope::Project, Scope::User] {
            let plan = build(&map, &set(&["judge"]), &f.target, &f.repo, scope, true).unwrap();
            match &plan.actions[0] {
                Action::RunInstaller { repo_root, .. } => assert_eq!(repo_root, &f.repo),
                other => panic!("expected RunInstaller at {scope:?}, got {other:?}"),
            }
        }
    }
```

Append to the `tests` module in `crates/wska/src/apply.rs`:

```rust
    #[test]
    fn an_installer_sees_the_repo_root_as_wska_repo() {
        let d = tempfile::tempdir().unwrap();
        let cwd = d.path().join("proj");
        fs::create_dir_all(&cwd).unwrap();
        let repo = d.path().join("repo");
        let plan = Plan {
            actions: vec![Action::RunInstaller {
                name: "env".into(),
                command: "printf %s \"$WSKA_REPO\" > marker".into(),
                cwd: cwd.clone(),
                repo_root: repo.clone(),
            }],
            refusals: vec![],
        };
        apply(&plan, false).unwrap();
        assert_eq!(
            fs::read_to_string(cwd.join("marker")).unwrap(),
            repo.to_string_lossy()
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `rtk cargo test -p wska`
Expected: compile errors, because `InstallerScope` and the `repo_root` field do not exist yet.

- [ ] **Step 3: Implement**

`crates/wska/src/config.rs`: add below `pub struct External {…}` and change `Installer`:

```rust
/// Where an installer runs. `Any` runs at whichever scope install is invoked
/// with: a machine-wide tool (judge, via `uv tool`) that a project-scoped
/// item needs would otherwise be skipped by the default `--scope project`.
/// Kept apart from `cli::Scope` so `--scope any` never becomes a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallerScope {
    Project,
    User,
    Any,
}

impl InstallerScope {
    pub fn admits(self, scope: Scope) -> bool {
        match self {
            InstallerScope::Any => true,
            InstallerScope::Project => scope == Scope::Project,
            InstallerScope::User => scope == Scope::User,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Installer {
    pub command: String,
    pub scope: InstallerScope,
}
```

In the existing test `parses_vendor_source_with_star_provides`, change the last assert to:

```rust
        assert_eq!(got.installers["bmad"].scope, InstallerScope::Project);
```

`crates/wska/src/item.rs`: replace `use crate::cli::Scope;` with `use crate::config::InstallerScope;`. In `Item::Installer`, change it to `scope: InstallerScope,`. In the tests, change `fn vendor_with_installer(name: &str, scope: Scope)` to `scope: InstallerScope`. Replace every `Scope::Project` in that test module with `InstallerScope::Project`: the calls at lines ~215, 252 and 274, and the assert at ~256.

`crates/wska/src/plan.rs`: add `use crate::config::InstallerScope;` inside `mod tests`. Add the field to the action:

```rust
    RunInstaller {
        name: String,
        command: String,
        cwd: PathBuf,
        /// This repo's root, exported to the installer as `WSKA_REPO` so its
        /// command can reference files here (`uv tool install "$WSKA_REPO/judge"`).
        repo_root: PathBuf,
    },
```

Replace the installer branch in `build`:

```rust
                if !want.admits(scope) {
                    plan.actions.push(Action::SkipInstaller {
                        name: name.clone(),
                        reason: format!("declared for {want:?} scope, invoked at {scope:?}"),
                    });
                } else if run_installers {
                    plan.actions.push(Action::RunInstaller {
                        name: name.clone(),
                        command: command.clone(),
                        cwd: target.parent().unwrap_or(target).to_path_buf(),
                        repo_root: repo_root.to_path_buf(),
                    });
                } else {
```

In `bmad_map()`, change it to `scope: InstallerScope::Project,`.

`crates/wska/src/apply.rs`: change the arm header to `Action::RunInstaller { name, command, cwd, repo_root } => {` and add `.env("WSKA_REPO", repo_root)` after `.current_dir(cwd)`. In the two existing tests that build `RunInstaller`, add `repo_root: d.path().to_path_buf(),`.

`crates/wska/src/doctor.rs` and `crates/wska/src/status.rs`: in each test module, replace `use crate::cli::Scope;` with `use crate::config::InstallerScope;`. Change `scope: Scope::Project,` to `scope: InstallerScope::Project,`.

- [ ] **Step 4: Run tests, clippy and fmt**

Run: `rtk cargo test -p wska && rtk cargo clippy --all-targets -- -D warnings && cargo fmt --all --check`
Expected: all pass. If clippy flags an unused `Scope` import, delete that import.

- [ ] **Step 5: Stage and hand off the commit**

```bash
rtk git add crates/wska/src/config.rs crates/wska/src/item.rs crates/wska/src/plan.rs crates/wska/src/apply.rs crates/wska/src/doctor.rs crates/wska/src/status.rs
cat > .task/commit-msg.txt <<'EOF'
feat(wska): any-scope installers and WSKA_REPO in installer env

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

Tell the user: `git commit -s -S -F .task/commit-msg.txt`.

---

### Task 2: judge skeleton, report contract, backends

**Files:**
- Create: `judge/pyproject.toml`, `judge/src/judge/__init__.py`, `judge/src/judge/report.py`, `judge/src/judge/backend.py`
- Create: `judge/tests/test_report.py`, `judge/tests/test_backend.py`, `judge/tests/fixtures/jev/rfc-ready-pass.json`
- Generated: `judge/uv.lock`

**Interfaces:**
- Produces (`judge.report`): `finding(file, message, source, line=None, probability=None, **extra) -> dict`; `report(tool, items, judge_model, unjudged_reason, verdict=None, **extra) -> dict`; `worst(verdicts) -> str`; `exit_code(rep) -> int`; `to_text(rep) -> str`.
- Produces (`judge.backend`): `JudgeError(Exception)` (exit 2); `Unreachable(Exception)` (→ unjudged); `Judge` protocol (`model: str | None`, `ask(state, questions) -> dict[str, Answer]`); `JevBackend(client=None)`; `FakeBackend(fixture=None, raises=None)` with `.calls: list[tuple[object, list[str]]]` and `FakeBackend.load(path)`; `select_backend(env=os.environ) -> Judge | None`.
- Fixture format (wire shape): `{"model": str, "answers": {qid: raw_answer}, "by_state": [{"contains": str, "answers": {qid: raw_answer}}]}`. Rules whose `contains` appears in `json.dumps(state)` override in order.

- [ ] **Step 1: Write the package files**

`judge/pyproject.toml`:

```toml
[project]
name = "judge"
version = "0.1.0"
description = "Headless review gates: rfc-ready and doc-review"
requires-python = ">=3.12"
dependencies = ["typesafe-sdk==0.7.5", "pyyaml>=6.0.2"]

[project.scripts]
judge = "judge.cli:entry"

[dependency-groups]
dev = ["pytest>=8.4"]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/judge"]

[tool.pytest.ini_options]
testpaths = ["tests"]

[tool.ruff]
line-length = 100
```

`judge/src/judge/__init__.py`:

```python
"""Headless review gates. See docs/judge.md."""
```

- [ ] **Step 2: Write the failing tests**

`judge/tests/fixtures/jev/rfc-ready-pass.json`:

```json
{
  "model": "jev-1.13.0",
  "answers": {
    "block-quality": {
      "type": "score", "score": 2.88, "confidence": 0.9,
      "legend": {"0": "placeholder", "1": "bare none", "2": "reasoned none", "3": "substantive"},
      "probabilities": {"0": 0.0, "1": 0.02, "2": 0.08, "3": 0.9}
    },
    "security-enforced": {"type": "noul", "noul": 0.9},
    "negative-tests": {"type": "noul", "noul": 0.85},
    "safe-defaults": {"type": "noul", "noul": 0.8},
    "probed-numbers": {"type": "noul", "noul": 0.7}
  }
}
```

`judge/tests/test_report.py`:

```python
from judge.report import exit_code, finding, report, to_text, worst


def test_worst_wins_fail_over_unjudged_over_pass():
    assert worst(["pass", "unjudged"]) == "unjudged"
    assert worst(["unjudged", "fail", "pass"]) == "fail"
    assert worst([]) == "pass"


def test_exit_codes():
    assert [exit_code({"verdict": v}) for v in ("pass", "fail", "unjudged")] == [0, 1, 3]


def test_report_shape_and_verdict_from_items():
    rep = report("rfc-ready", [{"verdict": "pass"}, {"verdict": "fail"}], "jev-1", None)
    assert rep == {
        "tool": "rfc-ready", "report_version": 1, "verdict": "fail",
        "judge_model": "jev-1", "unjudged_reason": None,
        "items": [{"verdict": "pass"}, {"verdict": "fail"}],
    }


def test_explicit_verdict_and_extra_keys():
    rep = report("doc-review", [], None, "no key", verdict="unjudged", dropped=[])
    assert rep["verdict"] == "unjudged" and rep["dropped"] == []


def test_finding_carries_the_contract_fields():
    f = finding("a.md", "msg", "check:heading")
    assert f == {"file": "a.md", "line": None, "message": "msg",
                 "source": "check:heading", "probability": None}


def test_text_format_mentions_verdict_and_findings():
    item = {"file": "a.md", "verdict": "fail", "claim_trust": 0.5, "verdict_confidence": 1.0,
            "findings": [finding("a.md", "§9 missing", "check:heading")]}
    text = to_text(report("rfc-ready", [item], None, None))
    assert "FAIL" in text and "§9 missing" in text and "check:heading" in text
```

`judge/tests/test_backend.py`:

```python
import json
from pathlib import Path
from types import SimpleNamespace

import httpx2
import pytest
import typesafe_sdk as ts

from judge.backend import FakeBackend, JevBackend, JudgeError, Unreachable, select_backend

FIX = Path(__file__).parent / "fixtures" / "jev"
NOUL = ts.Noul(instructions="x")


def test_fake_returns_sdk_answer_types_and_records_calls():
    fake = FakeBackend.load(FIX / "rfc-ready-pass.json")
    got = fake.ask({"text": "s"}, {"block-quality": ts.Score(criteria=["a", "b", "c", "d"])})
    a = got["block-quality"]
    assert isinstance(a, ts.ScoreAnswer) and a.probabilities[3] == 0.9
    assert fake.model == "jev-1.13.0"
    assert fake.calls == [({"text": "s"}, ["block-quality"])]


def test_fake_by_state_overrides_when_state_contains_text():
    fake = FakeBackend({"answers": {"q": {"type": "noul", "noul": 0.9}},
                        "by_state": [{"contains": "§7", "answers": {"q": {"type": "noul", "noul": 0.1}}}]})
    assert fake.ask({"text": "§7 Security"}, {"q": NOUL})["q"].noul == 0.1
    assert fake.ask({"text": "§8"}, {"q": NOUL})["q"].noul == 0.9


def test_fake_can_raise():
    with pytest.raises(Unreachable):
        FakeBackend(raises=Unreachable("down")).ask({}, {"q": NOUL})


class _Client:
    def __init__(self, result=None, exc=None):
        self.result, self.exc = result, exc

    def system_one(self, **kw):
        if self.exc:
            raise self.exc
        return self.result


def test_jev_records_the_resolved_model():
    answer = ts.NoulAnswer(type="noul", noul=0.7)
    jev = JevBackend(client=_Client(SimpleNamespace(model="jev-1.13.0", answers={"q": answer})))
    assert jev.ask({}, {"q": NOUL}) == {"q": answer}
    assert jev.model == "jev-1.13.0"


@pytest.mark.parametrize("exc", [
    ts.TypeSafeRateLimitError(429, {}, httpx2.Headers()),
    ts.TypeSafeInternalServerError(529, {}, httpx2.Headers()),
    ts.TypeSafeAPIConnectionError("down"),
])
def test_transient_errors_are_unreachable(exc):
    with pytest.raises(Unreachable):
        JevBackend(client=_Client(exc=exc)).ask({}, {"q": NOUL})


@pytest.mark.parametrize("exc", [
    ts.TypeSafeBadRequestError(400, {}, httpx2.Headers()),
    ts.TypeSafeAuthenticationError(401, {}, httpx2.Headers()),
    ts.TypeSafeUnprocessableEntityError(422, {}, httpx2.Headers()),
    ts.TypeSafeError("bad question"),
])
def test_request_errors_are_bugs(exc):
    with pytest.raises(JudgeError):
        JevBackend(client=_Client(exc=exc)).ask({}, {"q": NOUL})


def test_missing_answer_is_a_bug():
    jev = JevBackend(client=_Client(SimpleNamespace(model="m", answers={})))
    with pytest.raises(JudgeError):
        jev.ask({}, {"q": NOUL})


def test_select_backend_needs_a_non_blank_key():
    assert select_backend({}) is None
    assert select_backend({"TYPESAFE_API_KEY": "  "}) is None


def test_fixture_is_valid_json():
    json.loads((FIX / "rfc-ready-pass.json").read_text())
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd judge && uv lock && rtk test uv run pytest`
Expected: `uv.lock` is created. Tests fail with `ModuleNotFoundError: judge.report` / `judge.backend`.

- [ ] **Step 4: Implement**

`judge/src/judge/report.py`:

```python
"""The report and exit-code contract every judge command shares."""

SEVERITY = ("pass", "unjudged", "fail")
EXIT = {"pass": 0, "fail": 1, "unjudged": 3}


def finding(file, message, source, line=None, probability=None, **extra) -> dict:
    return {"file": file, "line": line, "message": message, "source": source,
            "probability": probability, **extra}


def worst(verdicts) -> str:
    return max(verdicts, key=SEVERITY.index, default="pass")


def report(tool, items, judge_model, unjudged_reason, verdict=None, **extra) -> dict:
    return {
        "tool": tool,
        "report_version": 1,
        "verdict": verdict or worst(i["verdict"] for i in items),
        "judge_model": judge_model,
        "unjudged_reason": unjudged_reason,
        "items": items,
        **extra,
    }


def exit_code(rep) -> int:
    return EXIT[rep["verdict"]]


def to_text(rep) -> str:
    head = f"{rep['tool']}: {rep['verdict'].upper()}"
    if rep["judge_model"]:
        head += f" (judge {rep['judge_model']})"
    if rep["unjudged_reason"]:
        head += f" — unjudged: {rep['unjudged_reason']}"
    out = [head]
    for item in rep["items"]:
        if "findings" in item:
            out.append(f"  {item['file']}  {item['verdict']}  trust={_num(item['claim_trust'])}"
                       f"  confidence={_num(item['verdict_confidence'])}")
            out += [f"    {_line(f)}" for f in item["findings"]]
        else:
            out.append(f"  {_line(item)}")
    out += [f"  dropped ({f['dropped_because']}): {_line(f)}" for f in rep.get("dropped", [])]
    return "\n".join(out)


def _line(f) -> str:
    loc = f"{f['file']}:{f['line']}" if f["line"] else f["file"]
    p = f" p={f['probability']}" if f["probability"] is not None else ""
    return f"{f['source']}  {loc}  {f['message']}{p}"


def _num(v) -> str:
    return "-" if v is None else f"{v:.2f}"
```

`judge/src/judge/backend.py`:

```python
"""The one door to a judge. Pipelines call `ask` with fixed questions; which
backend answers is decided once, from the environment."""

import json
import os
from pathlib import Path
from typing import Protocol

import typesafe_sdk as ts
from pydantic import TypeAdapter


class JudgeError(Exception):
    """A bug to fix, not a judgement: bad rubric, bad key, bad tool output. Exit 2."""


class Unreachable(Exception):
    """Jev could not answer after the SDK's own retries. The verdict is unjudged."""


class Judge(Protocol):
    model: str | None

    def ask(self, state, questions: dict[str, ts.Question]) -> dict[str, ts.Answer]: ...


# Rate limits, 5xx (529 included) and network failures are the SDK's to retry;
# once it gives up, "could not judge" is the honest answer.
_TRANSIENT = (ts.TypeSafeRateLimitError, ts.TypeSafeInternalServerError,
              ts.TypeSafeAPIConnectionError)


class JevBackend:
    def __init__(self, client=None):
        self._client = client or ts.TypeSafeClient()
        self.model: str | None = None

    def ask(self, state, questions):
        try:
            resp = self._client.system_one(state=state, questions=questions, model="jev-latest")
        except _TRANSIENT as e:
            raise Unreachable(f"Jev unreachable: {e}") from e
        except ts.TypeSafeError as e:
            raise JudgeError(f"Jev refused the request: {e}") from e
        self.model = resp.model
        missing = set(questions) - set(resp.answers)
        if missing:
            raise JudgeError(f"Jev returned no answer for {sorted(missing)}")
        return {q: resp.answers[q] for q in questions}


_ANSWER = TypeAdapter(ts.Answer)


class FakeBackend:
    """Answers from a fixture in the API's wire shape, parsed by the SDK's own
    answer types, so a fixture saved from a live run replays as-is."""

    def __init__(self, fixture: dict | None = None, raises: Exception | None = None):
        self.fixture = fixture or {}
        self.raises = raises
        self.model: str | None = None
        self.calls: list[tuple[object, list[str]]] = []

    @classmethod
    def load(cls, path) -> "FakeBackend":
        return cls(json.loads(Path(path).read_text()))

    def ask(self, state, questions):
        self.calls.append((state, sorted(questions)))
        if self.raises:
            raise self.raises
        text = json.dumps(state, ensure_ascii=False)
        raw = dict(self.fixture.get("answers", {}))
        for rule in self.fixture.get("by_state", []):
            if rule["contains"] in text:
                raw.update(rule["answers"])
        self.model = self.fixture.get("model", "jev-fake")
        return {q: _ANSWER.validate_json(json.dumps(raw[q])) for q in questions}


def select_backend(env=os.environ) -> Judge | None:
    return JevBackend() if env.get("TYPESAFE_API_KEY", "").strip() else None
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd judge && rtk test uv run pytest`
Expected: all pass. If an SDK error constructor signature differs from the one the tests use, read `typesafe_sdk/_core/errors.py` in the venv and fix the test, not the mapping.

- [ ] **Step 6: Lint, stage, hand off the commit**

```bash
uvx ruff check judge && uvx ruff format judge
rtk git add judge/pyproject.toml judge/uv.lock judge/src judge/tests
cat > .task/commit-msg.txt <<'EOF'
feat(judge): report contract and Jev/fake backends

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 3: Question files

**Files:**
- Create: `judge/src/judge/questions.py`, `judge/src/judge/questions/rfc-ready.yaml`, `judge/src/judge/questions/finding-filter.yaml`
- Test: `judge/tests/test_questions.py`

**Interfaces:**
- Consumes: `JudgeError` from Task 2.
- Produces: `QuestionFile(questions: dict[str, ts.Question], extra: dict)` (frozen dataclass); `load(name: str) -> QuestionFile` (packaged file `questions/<name>.yaml`); `parse(text: str, origin: str) -> QuestionFile`; `names() -> list[str]`.
- YAML ids used later: `block-quality` (score, 4 criteria, lowest first), `security-enforced`, `negative-tests`, `safe-defaults`, `probed-numbers` (noul); `rfc-ready` extra keys `thresholds` (incl. `block-quality`), `weights`; `actionable` (noul, instructions contain `{id}`); `finding-filter` extra key `threshold`.

- [ ] **Step 1: Write the failing tests**

`judge/tests/test_questions.py`:

```python
import pytest
import typesafe_sdk as ts

from judge import questions
from judge.backend import JudgeError


@pytest.mark.parametrize("name", questions.names())
def test_every_packaged_file_builds_into_sdk_questions(name):
    qf = questions.load(name)
    assert qf.questions
    for q in qf.questions.values():
        assert isinstance(q, (ts.Choice, ts.Score, ts.Noul))


def test_rfc_ready_rubric_contract():
    qf = questions.load("rfc-ready")
    assert len(qf.questions["block-quality"].criteria) == 4
    ids = set(qf.questions)
    assert set(qf.extra["thresholds"]) <= ids
    assert set(qf.extra["weights"]) <= ids | {"symbols"}


def test_finding_filter_contract():
    qf = questions.load("finding-filter")
    assert "{id}" in qf.questions["actionable"].instructions
    assert 0 < qf.extra["threshold"] < 1


def test_noul_true_false_keys_survive_yaml_booleans():
    qf = questions.parse(
        "questions:\n  q:\n    type: noul\n    instructions: x\n"
        "    criteria:\n      true: yes it is\n      false: no it is not\n", "t")
    assert qf.questions["q"].criteria == {"true": "yes it is", "false": "no it is not"}


@pytest.mark.parametrize("text", [
    "not: [valid",                                       # YAML error
    "questions: {}",                                     # empty
    "questions:\n  q:\n    type: box\n",                 # unsupported type
    "questions:\n  q:\n    type: score\n    criteria: []\n",
    "questions:\n  q:\n    type: choice\n    criteria: {}\n",
    "questions:\n  q:\n    type: noul\n    bogus: 1\n",  # unknown field
])
def test_bad_files_are_judge_errors(text):
    with pytest.raises(JudgeError):
        questions.parse(text, "t")
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd judge && rtk test uv run pytest tests/test_questions.py`
Expected: FAIL, `cannot import name 'questions'`.

- [ ] **Step 3: Implement**

`judge/src/judge/questions.py`:

```python
"""Question files: fixed, versioned rubrics, loaded and validated at startup
so a malformed one is exit 2 before any request is sent."""

from dataclasses import dataclass
from importlib import resources

import typesafe_sdk as ts
import yaml

from judge.backend import JudgeError

_TYPES = {"choice": ts.Choice, "score": ts.Score, "noul": ts.Noul}


@dataclass(frozen=True)
class QuestionFile:
    questions: dict[str, ts.Question]
    extra: dict  # pipeline-specific keys beside the questions: thresholds, weights


def _dir():
    return resources.files("judge").joinpath("questions")


def names() -> list[str]:
    return sorted(p.name.removesuffix(".yaml") for p in _dir().iterdir() if p.name.endswith(".yaml"))


def load(name: str) -> QuestionFile:
    return parse(_dir().joinpath(f"{name}.yaml").read_text(encoding="utf-8"), f"{name}.yaml")


def parse(text: str, origin: str) -> QuestionFile:
    try:
        data = yaml.safe_load(text)
    except yaml.YAMLError as e:
        raise JudgeError(f"{origin}: {e}") from e
    if not isinstance(data, dict) or not isinstance(data.get("questions"), dict) or not data["questions"]:
        raise JudgeError(f"{origin}: needs a non-empty `questions` map")
    out = {}
    for qid, spec in data["questions"].items():
        kind = spec.get("type") if isinstance(spec, dict) else None
        cls = _TYPES.get(kind)
        if cls is None:
            raise JudgeError(f"{origin}: question {qid}: type must be choice, score or noul, got {kind!r}")
        fields = {k: v for k, v in spec.items() if k != "type"}
        if kind == "noul" and isinstance(fields.get("criteria"), dict):
            # YAML reads bare `true:`/`false:` keys as booleans.
            fields["criteria"] = {str(k).lower(): v for k, v in fields["criteria"].items()}
        if kind in ("choice", "score") and not fields.get("criteria"):
            raise JudgeError(f"{origin}: question {qid}: {kind} needs non-empty criteria")
        try:
            out[qid] = cls(**fields)
        except (ValueError, TypeError) as e:
            raise JudgeError(f"{origin}: question {qid}: {e}") from e
    return QuestionFile(out, {k: v for k, v in data.items() if k != "questions"})
```

`judge/src/judge/questions/rfc-ready.yaml`:

```yaml
# Rubric for `judge rfc-ready`. Fixed and versioned on purpose: the same
# questions, judged the same way, every run. Move a threshold here, not in
# code. The rules mechanised are batlehub-rfc's §4 (mandatory blocks) and §7
# (review order).
version: 1

questions:
  block-quality:
    type: score
    instructions: >-
      This is one mandatory block of an RFC (design document). Rate how well it
      is written. A block may validly say nothing applies, but only with a
      reason specific to this change.
    # Order matters: rfc_ready.LEVELS names these four, lowest first.
    criteria:
      - "placeholder: template text, angle-bracket stubs, N/A, or empty"
      - "bare none: says nothing applies without saying why"
      - "reasoned none: says nothing applies and gives a reason specific to this change"
      - "substantive: real content about this change"
  security-enforced:
    type: noul
    instructions: >-
      Every security claim in §7 names the code (file, function or test) that
      enforces it. Answer yes when §7 makes no security claim.
  negative-tests:
    type: noul
    instructions: >-
      Every threat row in §7's threat model has a matching negative test in
      §14: the bypass attempted, refused. Answer yes when §7 lists no threat.
  safe-defaults:
    type: noul
    instructions: >-
      Every new default and control in §4.1 and §10.2 is the safe side and
      controls fail closed, or the text argues explicitly why not.
  probed-numbers:
    type: noul
    instructions: >-
      Numbers in this RFC (sizes, latencies, counts, limits) come from stated
      probes or measurements, not estimates. Answer yes when it states none.

thresholds:
  # Probability that a block is at least "reasoned none".
  block-quality: 0.5
  security-enforced: 0.5
  negative-tests: 0.5
  safe-defaults: 0.5

# Claim trust: weighted mean of these. `symbols` is the share of code-like
# tokens that exist in the repo; the rest are Noul probabilities.
weights:
  symbols: 1
  security-enforced: 1
  negative-tests: 1
  safe-defaults: 1
  probed-numbers: 1
```

`judge/src/judge/questions/finding-filter.yaml`:

```yaml
# The finding filter. `{id}` is replaced with each finding's id; all findings
# go in one request, so each question must say which one it is about.
version: 1

questions:
  actionable:
    type: noul
    instructions: >-
      Finding {id}: its claim is true of the doc and code excerpts shown for
      it, which are the real file contents, and a maintainer would change
      something because of it.

threshold: 0.6
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd judge && rtk test uv run pytest`
Expected: all pass.

- [ ] **Step 5: Lint, stage, hand off the commit**

```bash
uvx ruff check judge && uvx ruff format judge
rtk git add judge/src/judge/questions.py judge/src/judge/questions judge/tests/test_questions.py
cat > .task/commit-msg.txt <<'EOF'
feat(judge): versioned question files for rfc-ready and the finding filter

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 4: RFC parsing and deterministic checks

**Files:**
- Create: `judge/src/judge/rfc.py`
- Create: `judge/tests/conftest.py`, `judge/tests/fixtures/rfc/ready.md`, `judge/tests/test_rfc.py`

**Interfaces:**
- Produces: `MANDATORY = ("6.P", "7", "8", "9", "10", "11")`; `Section(id, title, level, line, end, text)`. `line` is the 1-based heading line. `end` is the 0-based exclusive end of the body in `Rfc.lines`. `Rfc(path, title, header, sections, lines, prose)`, where `prose` is `lines` with fenced code blanked. `parse(path) -> Rfc` raises `UnicodeDecodeError`/`OSError`, and the caller wraps them. Check functions: `missing_headings(rfc) -> list[str]`, `open_questions(rfc) -> list[tuple[int, str]]`, `html_comments(rfc) -> list[tuple[int, str]]`, `symbols(rfc) -> list[tuple[int, str]]`, `symbol_exists(root, tok) -> bool`, `prose_text(rfc) -> str`.
- Produces (tests): fixture `repo` in `conftest.py`. It returns a `Path` to a git repo containing `src/thing.py` (with `load_config` and `max_entries`) and a copy of `ready.md` at `docs/rfc/0001-cache-eviction.md`, all `git add`ed. Fixture `ready_text` returns the text of `ready.md`.

- [ ] **Step 1: Write the fixtures**

`judge/tests/fixtures/rfc/ready.md` (the inner fence uses ```` ``` ````):

````markdown
# RFC 0001 — Cache eviction

| Field       | Value            |
| ----------- | ---------------- |
| Status      | In review        |
| Short       | Cache eviction   |
| Risk        | low              |

---

## 1. Summary

Evict cold entries; `load_config()` in `src/thing.py` reads the limit.

```markdown
## 9. Data — an example heading, not a section
`not_a_claim_xyz` is an example
1. an example list item
```

## 4. User-facing design

### 4.1 Configuration

`max_entries` defaults to 1000, the safe side: a smaller cache only costs hits.

## 6. Detailed design

### 6.P Proof against a real client

1. `curl -sI` on an evicted key returned 404, and on a hot key 200.

## 7. Security

Low. Eviction can only delete; `load_config()` caps the limit it reads.

## 8. Supply chain

None: eviction uses only the standard library, so no dependency is added.

## 9. Data and privacy

None: eviction stores no new datum, it only deletes existing entries.

## 10. Operability

### 10.2 Failure modes

If eviction fails the entry stays, which is the safe side.

## 11. Other concerns

Docs: handled in the readme. Accessibility, i18n: no UI, ruled out.

## 14. Test plan

- **Security**: a negative test evicting another tenant's key, refused.

## 15. Decisions and open questions

### Resolved

| # | Question | Decision |
| --- | --- | --- |
| 1 | LRU or LFU | **LRU.** Simpler. |

### Still open

## 16. Implementation phases

One phase.
````

`judge/tests/conftest.py`:

```python
import shutil
import subprocess
from pathlib import Path

import pytest

FIX = Path(__file__).parent / "fixtures"


@pytest.fixture
def ready_text() -> str:
    return (FIX / "rfc" / "ready.md").read_text()


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "thing.py").write_text("max_entries = 1000\n\ndef load_config():\n    pass\n")
    (tmp_path / "docs" / "rfc").mkdir(parents=True)
    shutil.copy(FIX / "rfc" / "ready.md", tmp_path / "docs" / "rfc" / "0001-cache-eviction.md")
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    return tmp_path


def write_rfc(repo: Path, text: str, name: str = "0002-x.md") -> Path:
    p = repo / "docs" / "rfc" / name
    p.write_text(text)
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
    return p
```

- [ ] **Step 2: Write the failing tests**

`judge/tests/test_rfc.py`:

```python
from conftest import write_rfc

from judge import rfc


def test_header_title_and_sections(repo):
    doc = rfc.parse(repo / "docs/rfc/0001-cache-eviction.md")
    assert doc.title == "RFC 0001 — Cache eviction"
    assert doc.header["Status"] == "In review"
    assert "Field" not in doc.header
    assert {"1", "4", "4.1", "6", "6.P", "7", "8", "9", "10", "10.2", "11", "14", "15"} <= set(doc.sections)
    assert doc.sections["7"].title == "Security"
    assert "Eviction can only delete" in doc.sections["7"].text
    assert "If eviction fails" in doc.sections["10"].text  # a ## section includes its ### subsections


def test_ready_rfc_passes_every_deterministic_check(repo):
    doc = rfc.parse(repo / "docs/rfc/0001-cache-eviction.md")
    assert rfc.missing_headings(doc) == []
    assert rfc.open_questions(doc) == []
    assert rfc.html_comments(doc) == []


def test_missing_mandatory_heading(repo, ready_text):
    doc = rfc.parse(write_rfc(repo, ready_text.replace("## 9. Data and privacy", "## Data")))
    assert rfc.missing_headings(doc) == ["9"]


def test_fenced_heading_and_item_are_ignored(repo, ready_text):
    # The fenced example in §1 holds "## 9. Data…" and "1. an example list item".
    text = ready_text.replace("## 9. Data and privacy\n", "").replace(
        "### Still open\n", "### Still open\n\n```\n1. fenced, not open\n```\n")
    doc = rfc.parse(write_rfc(repo, text))
    assert rfc.missing_headings(doc) == ["9"]
    assert rfc.open_questions(doc) == []


def test_open_questions_are_counted_with_lines(repo, ready_text):
    text = ready_text.replace("### Still open\n", "### Still open\n\n1. Shard the cache?\n- TTL too?\n")
    doc = rfc.parse(write_rfc(repo, text))
    got = rfc.open_questions(doc)
    assert [t for _, t in got] == ["1. Shard the cache?", "- TTL too?"]
    assert doc.lines[got[0][0] - 1] == "1. Shard the cache?"


def test_html_comments_each_reported_even_unclosed(repo, ready_text):
    text = ready_text.replace("## 8. Supply chain\n", "## 8. Supply chain\n<!-- MANDATORY.\nfill -->\n") + "\n<!-- left open"
    got = rfc.html_comments(rfc.parse(write_rfc(repo, text)))
    assert len(got) == 2 and got[0][1].startswith("MANDATORY")


def test_symbol_extraction_skips_fences_commands_and_prose(repo):
    toks = [t for _, t in rfc.symbols(rfc.parse(repo / "docs/rfc/0001-cache-eviction.md"))]
    assert toks == ["load_config()", "src/thing.py", "max_entries"]


def test_looks_like_code():
    yes = ["a::b", "src/x.rs", "run()", "snake_case", "JevBackend"]
    no = ["curl -sI", "ab", "Rfc", "README", "plain", "crates/…"]
    assert all(rfc.looks_like_code(t) for t in yes)
    assert not any(rfc.looks_like_code(t) for t in no)


def test_symbol_exists_in_code_and_paths(repo):
    assert rfc.symbol_exists(repo, "load_config()")
    assert rfc.symbol_exists(repo, "src/thing.py")
    assert not rfc.symbol_exists(repo, "no_such_symbol")


def test_symbol_only_in_markdown_is_missing(repo, ready_text):
    write_rfc(repo, ready_text + "\n`only_in_docs_xyz`\n")
    (repo / "docs" / "other.md").write_text("only_in_docs_xyz\n")
    assert not rfc.symbol_exists(repo, "only_in_docs_xyz")
```

- [ ] **Step 3: Run to verify they fail**

Run: `cd judge && rtk test uv run pytest tests/test_rfc.py`
Expected: FAIL, `cannot import name 'rfc'`.

- [ ] **Step 4: Implement**

`judge/src/judge/rfc.py`:

```python
"""Parse a batlehub-rfc document and run the deterministic checks. Fenced
code is never a claim: examples are not headings, questions or symbols."""

import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

MANDATORY = ("6.P", "7", "8", "9", "10", "11")

_HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
_NUMBERED = re.compile(r"^(\d+(?:\.(?:\d+|P))?)\.?\s+(.*)$")
_FENCE = re.compile(r"^\s*(```|~~~)")
_ROW = re.compile(r"^\|\s*([^|]+?)\s*\|\s*(.*?)\s*\|\s*$")
_ITEM = re.compile(r"^\s*(?:\d+\.|[-*])\s+\S")
_COMMENT = re.compile(r"<!--.*?(?:-->|\Z)", re.S)
_TICKED = re.compile(r"`([^`\n]+)`")
_SNAKE = re.compile(r"^[a-z][a-z0-9]*(?:_[a-z0-9]+)+$")
_CAMEL = re.compile(r"^[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]*)+$")
_PATHLIKE = re.compile(r"/.*\.[A-Za-z0-9]+$")


@dataclass
class Section:
    id: str
    title: str
    level: int
    line: int  # 1-based line of the heading
    end: int  # 0-based exclusive end of the body in Rfc.lines
    text: str


@dataclass
class Rfc:
    path: Path
    title: str
    header: dict[str, str]
    sections: dict[str, Section]
    lines: list[str]
    prose: list[str]  # lines with fenced code blanked, same length


def _fence_mask(lines):
    # shortcut: a ```` fence holding a ``` line toggles early; upgrade if RFCs nest fences.
    mask, inside = [], False
    for line in lines:
        if _FENCE.match(line):
            mask.append(True)
            inside = not inside
        else:
            mask.append(inside)
    return mask


def parse(path: Path) -> Rfc:
    lines = path.read_text(encoding="utf-8").splitlines()
    prose = ["" if fenced else line for line, fenced in zip(lines, _fence_mask(lines))]
    headings = [(i, len(m.group(1)), m.group(2)) for i, m in
                ((i, _HEADING.match(line)) for i, line in enumerate(prose)) if m]
    title = next((t for _, lvl, t in headings if lvl == 1), path.stem)
    sections: dict[str, Section] = {}
    for n, (i, lvl, text) in enumerate(headings):
        m = _NUMBERED.match(text)
        if not m:
            continue
        end = next((j for j, l2, _ in headings[n + 1:] if l2 <= lvl), len(lines))
        sections.setdefault(m.group(1), Section(m.group(1), m.group(2), lvl, i + 1, end,
                                                "\n".join(lines[i + 1:end]).strip()))
    first_section = next((i for i, lvl, _ in headings if lvl == 2), len(lines))
    header = {}
    for line in prose[:first_section]:
        m = _ROW.match(line)
        if m and m.group(1) != "Field" and not set(m.group(1)) <= set("-: "):
            header[m.group(1)] = m.group(2)
    return Rfc(path, title, header, sections, lines, prose)


def _uncommented(rfc: Rfc) -> list[str]:
    text = _COMMENT.sub(lambda m: "\n" * m.group().count("\n"), "\n".join(rfc.prose))
    return text.split("\n")


def prose_text(rfc: Rfc) -> str:
    return "\n".join(rfc.prose).strip()


def missing_headings(rfc: Rfc) -> list[str]:
    return [s for s in MANDATORY if s not in rfc.sections]


def open_questions(rfc: Rfc) -> list[tuple[int, str]]:
    sec = rfc.sections.get("15")
    if sec is None:
        return []
    body, out, inside = _uncommented(rfc), [], False
    for i in range(sec.line, sec.end):
        h = _HEADING.match(body[i])
        if h:
            inside = h.group(2).strip().lower() == "still open"
        elif inside and _ITEM.match(body[i]):
            out.append((i + 1, body[i].strip()))
    return out


def html_comments(rfc: Rfc) -> list[tuple[int, str]]:
    text = "\n".join(rfc.prose)
    return [(text.count("\n", 0, m.start()) + 1,
             " ".join(m.group().removeprefix("<!--").removesuffix("-->").split())[:80])
            for m in _COMMENT.finditer(text)]


def looks_like_code(tok: str) -> bool:
    if " " in tok or len(tok) < 3:
        return False
    return ("::" in tok or "()" in tok or bool(_PATHLIKE.search(tok))
            or bool(_SNAKE.match(tok)) or bool(_CAMEL.match(tok)))


def symbols(rfc: Rfc) -> list[tuple[int, str]]:
    seen, out = set(), []
    for i, line in enumerate(_uncommented(rfc)):
        for m in _TICKED.finditer(line):
            tok = m.group(1).strip()
            if looks_like_code(tok) and tok not in seen:
                seen.add(tok)
                out.append((i + 1, tok))
    return out


def symbol_exists(root: Path, tok: str) -> bool:
    if _PATHLIKE.search(tok) and "::" not in tok and (root / tok.lstrip("/")).exists():
        return True
    # Markdown is excluded: the RFC naming a symbol is not evidence it exists.
    # shortcut: `a::b` is grepped verbatim, so `use a::{b}` reads as missing; trust only, never gates.
    needle = tok.removesuffix("()")
    return subprocess.run(["git", "grep", "-F", "-q", "-e", needle, "--", ".", ":(exclude)*.md"],
                          cwd=root, capture_output=True).returncode == 0
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd judge && rtk test uv run pytest`
Expected: all pass.

- [ ] **Step 6: Lint, stage, hand off the commit**

```bash
uvx ruff check judge && uvx ruff format judge
rtk git add judge/src/judge/rfc.py judge/tests/conftest.py judge/tests/fixtures/rfc judge/tests/test_rfc.py
cat > .task/commit-msg.txt <<'EOF'
feat(judge): RFC parsing and deterministic readiness checks

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 5: rfc-ready pipeline and scoring

**Files:**
- Create: `judge/src/judge/rfc_ready.py`
- Test: `judge/tests/test_rfc_ready.py`

**Interfaces:**
- Consumes: `rfc.*` (Task 4), `questions.load` (Task 3), `FakeBackend`, `JudgeError`, `Unreachable`, `report`, `finding` (Task 2); fixture `jev/rfc-ready-pass.json`.
- Produces: `run(files: list[Path], root: Path, backend: Judge | None) -> dict` (a report); `judge_one(doc, root, backend, qf) -> tuple[dict, str | None]`; `discover(root) -> list[Path]`; `LEVELS`.
- Item keys: `file` (relative to root when inside it), `status`, `verdict`, `claim_trust`, `verdict_confidence`, `findings`.

Scoring rules (from the spec):
- Gate per block: P(level ≥ `reasoned none`) ≥ `thresholds.block-quality`. Gate per Noul in `security-enforced`, `negative-tests`, `safe-defaults`: probability ≥ its threshold.
- Certainty of an answer: for a Score, the returned `confidence`; for a Noul, `2·|p − 0.5|`.
- Verdict confidence: 1.0 when a deterministic check failed; `null` when unjudged; otherwise the minimum certainty among the failing gates if any failed, else among all gates.
- Claim trust: weighted mean of whichever of `symbols`, `security-enforced`, `negative-tests`, `safe-defaults`, `probed-numbers` are available; `null` if none are.
- Jev is skipped entirely when a mandatory heading is missing.

- [ ] **Step 1: Write the failing tests**

`judge/tests/test_rfc_ready.py`:

```python
import json
from pathlib import Path

import pytest
from conftest import write_rfc

from judge import rfc_ready
from judge.backend import FakeBackend, JudgeError, Unreachable

PASS = json.loads((Path(__file__).parent / "fixtures/jev/rfc-ready-pass.json").read_text())


def fake(**by_state) -> FakeBackend:
    fx = json.loads(json.dumps(PASS))
    fx["by_state"] = [{"contains": k, "answers": v} for k, v in by_state.items()]
    return FakeBackend(fx)


def one(repo, backend, path="docs/rfc/0001-cache-eviction.md"):
    rep = rfc_ready.run([repo / path], repo, backend)
    return rep, rep["items"][0]


def test_ready_rfc_passes_with_trust_and_confidence(repo):
    rep, item = one(repo, fake())
    assert rep["verdict"] == "pass" and rep["judge_model"] == "jev-1.13.0"
    assert item["file"] == "docs/rfc/0001-cache-eviction.md" and item["status"] == "In review"
    assert item["findings"] == []
    assert item["claim_trust"] == pytest.approx((1.0 + 0.9 + 0.85 + 0.8 + 0.7) / 5, abs=1e-3)
    # Gating certainties: block-quality 0.9; Nouls 2·|p−0.5| = 0.8, 0.7, 0.6. probed-numbers does not gate.
    assert item["verdict_confidence"] == pytest.approx(0.6, abs=1e-3)


def test_missing_block_fails_without_calling_jev(repo, ready_text):
    backend = fake()
    p = write_rfc(repo, ready_text.replace("## 9. Data and privacy", "## Data"))
    rep, item = one(repo, backend, p.relative_to(repo))
    assert rep["verdict"] == "fail" and item["verdict_confidence"] == 1.0
    assert [f["source"] for f in item["findings"]] == ["check:heading"]
    assert backend.calls == []


def test_filler_in_security_fails_on_block_quality(repo):
    filler = {"block-quality": {"type": "score", "score": 0.2, "confidence": 0.8,
              "legend": {"0": "p", "1": "b", "2": "r", "3": "s"},
              "probabilities": {"0": 0.85, "1": 0.1, "2": 0.05, "3": 0.0}}}
    rep, item = one(repo, fake(**{"§7 Security": filler}))
    assert rep["verdict"] == "fail"
    (f,) = item["findings"]
    assert f["source"] == "jev:block-quality" and "§7" in f["message"] and "placeholder" in f["message"]
    assert f["probability"] == pytest.approx(0.05)
    assert item["verdict_confidence"] == pytest.approx(0.8)


def test_borderline_noul_passes_with_low_confidence(repo):
    rep, item = one(repo, fake(**{"§4.1": {"safe-defaults": {"type": "noul", "noul": 0.52}}}))
    assert rep["verdict"] == "pass"
    assert item["verdict_confidence"] == pytest.approx(0.04, abs=1e-3)


def test_no_key_is_unjudged_with_deterministic_findings_kept(repo, ready_text):
    p = write_rfc(repo, ready_text + "\n<!-- template leftover -->\n")
    rep = rfc_ready.run([repo / "docs/rfc/0001-cache-eviction.md", p], repo, None)
    ready, broken = rep["items"]
    assert ready["verdict"] == "unjudged" and ready["verdict_confidence"] is None
    assert ready["claim_trust"] == 1.0  # symbols only
    assert broken["verdict"] == "fail" and broken["findings"][0]["source"] == "check:template-comment"
    assert rep["verdict"] == "fail" and rep["unjudged_reason"] == "no TYPESAFE_API_KEY"


def test_unreachable_is_unjudged_with_reason(repo):
    rep, item = one(repo, FakeBackend(raises=Unreachable("Jev unreachable: 529")))
    assert rep["verdict"] == "unjudged" and "529" in rep["unjudged_reason"]


def test_missing_symbols_are_findings_but_never_gate(repo, ready_text):
    p = write_rfc(repo, ready_text.replace("Evict cold entries;", "Evict `ghost_symbol` cold entries;"))
    rep, item = one(repo, fake(), p.relative_to(repo))
    assert rep["verdict"] == "pass"
    assert [f["source"] for f in item["findings"]] == ["check:symbols"]
    assert item["claim_trust"] < (1.0 + 0.9 + 0.85 + 0.8 + 0.7) / 5


def test_discover_skips_implemented_and_rejected(repo, ready_text):
    write_rfc(repo, ready_text.replace("In review", "Implemented"), "0002-done.md")
    write_rfc(repo, ready_text.replace("In review", "Rejected"), "0003-no.md")
    assert [p.name for p in rfc_ready.discover(repo)] == ["0001-cache-eviction.md"]


def test_named_file_is_judged_whatever_its_status(repo, ready_text):
    p = write_rfc(repo, ready_text.replace("In review", "Implemented"), "0002-done.md")
    rep = rfc_ready.run([p], repo, None)
    assert len(rep["items"]) == 1


def test_no_rfcs_is_a_pass(tmp_path):
    rep = rfc_ready.run([], tmp_path, None)
    assert rep["verdict"] == "pass" and rep["items"] == []


def test_missing_or_undecodable_file_is_a_judge_error(repo):
    with pytest.raises(JudgeError):
        rfc_ready.run([repo / "nope.md"], repo, None)
    bad = repo / "docs/rfc/bad.md"
    bad.write_bytes(b"\xff\xfe\x00")
    with pytest.raises(JudgeError):
        rfc_ready.run([bad], repo, None)
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd judge && rtk test uv run pytest tests/test_rfc_ready.py`
Expected: FAIL, `cannot import name 'rfc_ready'`.

- [ ] **Step 3: Implement**

`judge/src/judge/rfc_ready.py`:

```python
"""rfc-ready: is an RFC ready for sign-off, how far can its claims be trusted,
and how sure is that verdict. Deterministic checks first; Jev only for what
needs judgement. The rules are batlehub-rfc's §4 and §7; this mechanises them."""

from pathlib import Path

from judge import questions
from judge import rfc as rfcmod
from judge.backend import JudgeError, Unreachable
from judge.report import finding, report

LEVELS = ("placeholder", "bare none", "reasoned none", "substantive")  # rfc-ready.yaml order
READY = LEVELS.index("reasoned none")
GATES = ("security-enforced", "negative-tests", "safe-defaults")
TRUST = ("security-enforced", "negative-tests", "safe-defaults", "probed-numbers")
SKIP = {"implemented", "rejected"}
MESSAGES = {
    "security-enforced": "a security claim in §7 names no code that enforces it",
    "negative-tests": "a threat in §7 has no negative test in §14",
    "safe-defaults": "a new default or control in §4.1/§10.2 is not the safe side, and the text does not argue why",
}


def _parse(path: Path) -> rfcmod.Rfc:
    if not path.is_file():
        raise JudgeError(f"no such RFC: {path}")
    try:
        return rfcmod.parse(path)
    except (OSError, UnicodeDecodeError) as e:
        raise JudgeError(f"cannot read {path}: {e}") from e


def discover(root: Path) -> list[Path]:
    return [p for p in sorted((root / "docs" / "rfc").glob("*.md"))
            if _parse(p).header.get("Status", "").strip().lower() not in SKIP]


def run(files, root: Path, backend) -> dict:
    qf = questions.load("rfc-ready")
    items, reasons = [], []
    for path in [Path(f) for f in files] or discover(root):
        item, reason = judge_one(_parse(path), root, backend, qf)
        items.append(item)
        if reason and reason not in reasons:
            reasons.append(reason)
    model = backend.model if backend else None
    return report("rfc-ready", items, model, "; ".join(reasons) or None)


def _rel(path: Path, root: Path) -> str:
    path = path.resolve()
    return str(path.relative_to(root.resolve())) if path.is_relative_to(root.resolve()) else str(path)


def _state(doc, part, *ids):
    text = "\n\n".join(
        f"§{i} {doc.sections[i].title}\n{doc.sections[i].text}" if i in doc.sections
        else f"§{i}: absent from this RFC" for i in ids)
    return {"rfc": doc.title, "part": part, "text": text}


def _ask(doc, backend, qf):
    q = qf.questions
    blocks = {b: backend.ask(_state(doc, f"§{b}", b), {"block-quality": q["block-quality"]})["block-quality"]
              for b in rfcmod.MANDATORY}
    nouls = {}
    whole = {"rfc": doc.title, "part": "whole RFC, code blocks stripped", "text": rfcmod.prose_text(doc)}
    for state, ids in ((_state(doc, "§7 + §14", "7", "14"), ("security-enforced", "negative-tests")),
                       (_state(doc, "§4.1 + §10.2", "4.1", "10.2"), ("safe-defaults",)),
                       (whole, ("probed-numbers",))):
        got = backend.ask(state, {i: q[i] for i in ids})
        nouls.update({i: got[i].noul for i in ids})
    return blocks, nouls


def judge_one(doc, root: Path, backend, qf) -> tuple[dict, str | None]:
    rel = _rel(doc.path, root)
    missing = rfcmod.missing_headings(doc)
    findings = [finding(rel, f"mandatory block §{s} is missing", "check:heading") for s in missing]
    findings += [finding(rel, f"question still open: {t}", "check:open-questions", line)
                 for line, t in rfcmod.open_questions(doc)]
    findings += [finding(rel, f"template comment left in: {t}", "check:template-comment", line)
                 for line, t in rfcmod.html_comments(doc)]
    deterministic_fail = bool(findings)

    syms = rfcmod.symbols(doc)
    absent = [(line, tok) for line, tok in syms if not rfcmod.symbol_exists(root, tok)]
    findings += [finding(rel, f"`{tok}` not found in the repo", "check:symbols", line) for line, tok in absent]
    trust = {"symbols": 1 - len(absent) / len(syms)} if syms else {}

    reason, deciding = None, []  # deciding: (passed, certainty) per gating answer
    if missing:
        pass  # a missing block needs no judge
    elif backend is None:
        reason = "no TYPESAFE_API_KEY"
    else:
        try:
            blocks, nouls = _ask(doc, backend, qf)
        except Unreachable as e:
            reason = str(e)
        else:
            th = qf.extra.get("thresholds", {})
            for b, a in blocks.items():
                p = sum(v for lvl, v in a.probabilities.items() if lvl >= READY)
                ok = p >= th.get("block-quality", 0.5)
                deciding.append((ok, a.confidence))
                if not ok:
                    level = LEVELS[max(a.probabilities, key=a.probabilities.get)]
                    findings.append(finding(rel, f"§{b} reads as '{level}', below 'reasoned none'",
                                            "jev:block-quality", doc.sections[b].line, round(p, 3)))
            for q in GATES:
                ok = nouls[q] >= th.get(q, 0.5)
                deciding.append((ok, 2 * abs(nouls[q] - 0.5)))
                if not ok:
                    findings.append(finding(rel, MESSAGES[q], f"jev:{q}", None, round(nouls[q], 3)))
            trust.update({q: nouls[q] for q in TRUST})

    failed = [c for ok, c in deciding if not ok]
    if deterministic_fail:
        verdict, confidence = "fail", 1.0
    elif reason:
        verdict, confidence = "unjudged", None
    elif failed:
        verdict, confidence = "fail", min(failed)
    else:
        verdict, confidence = "pass", min(c for _, c in deciding)

    weights = qf.extra.get("weights", {})
    total = sum(weights.get(k, 1) for k in trust)
    claim_trust = round(sum(weights.get(k, 1) * v for k, v in trust.items()) / total, 3) if total else None
    item = {"file": rel, "status": doc.header.get("Status"), "verdict": verdict,
            "claim_trust": claim_trust,
            "verdict_confidence": None if confidence is None else round(confidence, 3),
            "findings": findings}
    return item, reason
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd judge && rtk test uv run pytest`
Expected: all pass.

- [ ] **Step 5: Lint, stage, hand off the commit**

```bash
uvx ruff check judge && uvx ruff format judge
rtk git add judge/src/judge/rfc_ready.py judge/tests/test_rfc_ready.py
cat > .task/commit-msg.txt <<'EOF'
feat(judge): rfc-ready pipeline with claim trust and verdict confidence

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 6: Excerpts, finding schema and the filter

**Files:**
- Create: `judge/src/judge/excerpt.py`, `judge/src/judge/filter.py`, `judge/src/judge/schemas/findings.schema.json`
- Test: `judge/tests/test_excerpt.py`, `judge/tests/test_filter.py`

**Interfaces:**
- Consumes: `questions.load("finding-filter")`, `FakeBackend`, `JudgeError`, `Unreachable`, `finding`, `report`; fixture `repo` (Task 4).
- Produces: `excerpt(root: Path, file: str, line: int | None, radius: int = 10) -> str | None`. It returns `None` when the file is untracked, outside root, missing, or the line is past EOF. `KINDS`; `parse_findings(obj) -> list[dict]`, which raises `JudgeError`; `run(findings, root, backend, tool="filter") -> dict`. The report's `items` are kept findings and `dropped` holds dropped ones, each with `dropped_because` set to `fabricated-ref` or `below-threshold`. Question ids sent to Jev are `f<index in input>`.
- Finding output: `file`, `line`, `message` (the claim), `source` (`jev:actionable` when judged, else `agent`), `probability`, `kind`, `code_ref`, `suggestion`.

- [ ] **Step 1: Write the failing tests**

`judge/tests/test_excerpt.py`:

```python
import subprocess

from judge.excerpt import excerpt


def test_excerpt_window_is_numbered_and_clamped(repo):
    got = excerpt(repo, "src/thing.py", 1, radius=1)
    assert got.splitlines() == ["    1| max_entries = 1000", "    2| "]


def test_no_line_means_the_head_of_the_file(repo):
    assert excerpt(repo, "src/thing.py", None).startswith("    1| max_entries")


def test_line_past_eof_or_missing_file_is_none(repo):
    assert excerpt(repo, "src/thing.py", 999) is None
    assert excerpt(repo, "src/nope.py", 1) is None


def test_excerpt_refuses_untracked_and_outside_paths(repo, tmp_path_factory):
    (repo / ".env").write_text("TOKEN=secret\n")
    (repo / ".gitignore").write_text(".env\n")
    subprocess.run(["git", "add", ".gitignore"], cwd=repo, check=True)
    outside = tmp_path_factory.mktemp("out") / "x.txt"
    outside.write_text("x\n")
    for path in [".env", str(outside), "../x.txt", ".git/config", "/etc/passwd"]:
        assert excerpt(repo, path, 1) is None, path
```

`judge/tests/test_filter.py`:

```python
import json
from importlib import resources

import pytest

from judge import filter as filter_
from judge.backend import FakeBackend, JudgeError, Unreachable


def f(file="src/thing.py", line=1, ref=None, kind="missing"):
    return {"file": file, "line": line, "kind": kind, "claim": "undocumented",
            "code_ref": ref, "suggestion": "document it"}


def yes(p):
    return {"type": "noul", "noul": p}


def test_schema_and_validator_agree_on_kinds():
    schema = json.loads(resources.files("judge").joinpath("schemas/findings.schema.json").read_text())
    kinds = schema["properties"]["findings"]["items"]["properties"]["kind"]["enum"]
    assert tuple(kinds) == filter_.KINDS


@pytest.mark.parametrize("obj", [
    [], {"findings": "x"}, {"findings": [{"file": "a"}]},
    {"findings": [f(kind="other")]}, {"findings": [f(line=0)]}, {"findings": [f(line=True)]},
    {"findings": [f(ref={"file": ""})]},
])
def test_malformed_findings_are_judge_errors(obj):
    with pytest.raises(JudgeError):
        filter_.parse_findings(obj)


def test_keeps_actionable_drops_weak_and_fabricated(repo):
    findings = [f(), f(line=999), f(ref={"file": "src/thing.py", "line": 3})]
    backend = FakeBackend({"answers": {"f0": yes(0.9), "f2": yes(0.3)}})
    rep = filter_.run(findings, repo, backend, tool="doc-review")
    assert rep["verdict"] == "fail" and rep["tool"] == "doc-review"
    assert [(x["source"], x["probability"]) for x in rep["items"]] == [("jev:actionable", 0.9)]
    assert [(x["dropped_because"], x["probability"]) for x in rep["dropped"]] == [
        ("fabricated-ref", None), ("below-threshold", 0.3)]
    (call,) = backend.calls  # one batched request
    assert call[1] == ["f0", "f2"]


def test_state_holds_real_excerpts_not_agent_quotes(repo):
    backend = FakeBackend({"answers": {"f0": yes(0.9)}})
    finding = f() | {"claim": "the file says HELLO"}
    filter_.run([finding], repo, backend)
    state = backend.calls[0][0]
    assert "max_entries = 1000" in state["findings"]["f0"]["doc_excerpt"]


def test_filter_never_sends_untracked_content(repo):
    (repo / ".env").write_text("TOKEN=secret\n")
    backend = FakeBackend({"answers": {}})
    rep = filter_.run([f(file=".env")], repo, backend)
    assert backend.calls == [] and rep["dropped"][0]["dropped_because"] == "fabricated-ref"
    assert rep["verdict"] == "pass"


def test_no_key_reports_survivors_unfiltered(repo):
    rep = filter_.run([f(), f(line=999)], repo, None)
    assert rep["verdict"] == "unjudged" and rep["unjudged_reason"] == "no TYPESAFE_API_KEY"
    assert [(x["source"], x["probability"]) for x in rep["items"]] == [("agent", None)]
    assert len(rep["dropped"]) == 1


def test_unreachable_is_unjudged(repo):
    rep = filter_.run([f()], repo, FakeBackend(raises=Unreachable("down")))
    assert rep["verdict"] == "unjudged" and rep["unjudged_reason"] == "down"


def test_nothing_found_is_a_pass(repo):
    assert filter_.run([], repo, None)["verdict"] == "pass"
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd judge && rtk test uv run pytest tests/test_excerpt.py tests/test_filter.py`
Expected: FAIL on import errors.

- [ ] **Step 3: Implement**

`judge/src/judge/excerpt.py`:

```python
"""Re-read what a finding cites, from disk. The agent's own quotes are never
trusted, and only files the repo tracks are read: an agent pointing at
`.env` or outside the root must not get its content sent to Jev."""

import subprocess
from pathlib import Path


def excerpt(root: Path, file: str, line: int | None, radius: int = 10) -> str | None:
    root = root.resolve()
    path = (root / file).resolve()
    if not path.is_relative_to(root) or not path.is_file():
        return None
    rel = str(path.relative_to(root))
    tracked = subprocess.run(["git", "ls-files", "--error-unmatch", "--", rel],
                             cwd=root, capture_output=True).returncode == 0
    if not tracked:
        return None
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None
    if line is None:
        line = 1
    elif line > len(lines):
        return None
    lo, hi = max(1, line - radius), min(len(lines), line + radius)
    return "\n".join(f"{n:>5}| {lines[n - 1]}" for n in range(lo, hi + 1))
```

`judge/src/judge/schemas/findings.schema.json`:

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["findings"],
  "properties": {
    "findings": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["file", "line", "kind", "claim", "code_ref", "suggestion"],
        "properties": {
          "file": {"type": "string", "minLength": 1},
          "line": {"type": ["integer", "null"], "minimum": 1},
          "kind": {"enum": ["missing", "stale", "wrong", "unclear"]},
          "claim": {"type": "string", "minLength": 1},
          "code_ref": {
            "anyOf": [
              {"type": "null"},
              {
                "type": "object",
                "additionalProperties": false,
                "required": ["file", "line"],
                "properties": {
                  "file": {"type": "string", "minLength": 1},
                  "line": {"type": ["integer", "null"], "minimum": 1}
                }
              }
            ]
          },
          "suggestion": {"type": "string"}
        }
      }
    }
  }
}
```

`judge/src/judge/filter.py`:

```python
"""The finding filter, shared by every reviewer: drop findings that cite
nothing real, then keep only those Jev judges actionable. Dropped findings
stay in the report so the filter itself can be audited."""

from judge import questions
from judge.backend import JudgeError, Unreachable
from judge.excerpt import excerpt
from judge.report import finding, report

KINDS = ("missing", "stale", "wrong", "unclear")  # = findings.schema.json


def _text(v):
    return isinstance(v, str) and v.strip() != ""


def _line(v):
    return v is None or (isinstance(v, int) and not isinstance(v, bool) and v >= 1)


def parse_findings(obj) -> list[dict]:
    """Validate at the trust boundary: this is LLM output."""
    if not isinstance(obj, dict) or not isinstance(obj.get("findings"), list):
        raise JudgeError("findings must be an object with a `findings` list")
    for n, f in enumerate(obj["findings"]):
        ref = f.get("code_ref") if isinstance(f, dict) else None
        ok = (isinstance(f, dict) and _text(f.get("file")) and _line(f.get("line"))
              and f.get("kind") in KINDS and _text(f.get("claim"))
              and isinstance(f.get("suggestion", ""), str)
              and (ref is None or (isinstance(ref, dict) and _text(ref.get("file"))
                                   and _line(ref.get("line")))))
        if not ok:
            raise JudgeError(f"finding {n} does not match findings.schema.json: {f!r:.200}")
    return obj["findings"]


def _out(f, p, why=None) -> dict:
    out = finding(f["file"], f["claim"], "jev:actionable" if p is not None else "agent",
                  f.get("line"), p, kind=f["kind"], code_ref=f.get("code_ref"),
                  suggestion=f.get("suggestion", ""))
    if why:
        out["dropped_because"] = why
    return out


def run(findings, root, backend, tool="filter") -> dict:
    qf = questions.load("finding-filter")
    threshold = qf.extra.get("threshold", 0.6)
    base = qf.questions["actionable"]
    kept, dropped, survivors = [], [], {}
    for n, f in enumerate(findings):
        doc = excerpt(root, f["file"], f.get("line"))
        ref = f.get("code_ref")
        code = excerpt(root, ref["file"], ref.get("line")) if ref else None
        if doc is None or (ref and code is None):
            dropped.append(_out(f, None, "fabricated-ref"))
            continue
        survivors[f"f{n}"] = (f, {"kind": f["kind"], "claim": f["claim"],
                                  "suggestion": f.get("suggestion", ""),
                                  "doc_excerpt": doc, "code_excerpt": code})

    reason, model, got = None, None, {}
    if survivors and backend is None:
        reason = "no TYPESAFE_API_KEY"
    elif survivors:
        state = {"findings": {fid: s for fid, (_, s) in survivors.items()}}
        qs = {fid: base.model_copy(update={"instructions": base.instructions.replace("{id}", fid)})
              for fid in survivors}
        try:
            got = backend.ask(state, qs)
            model = backend.model
        except Unreachable as e:
            reason = str(e)

    for fid, (f, _) in survivors.items():
        if reason:
            kept.append(_out(f, None))
            continue
        p = round(got[fid].noul, 3)
        if p >= threshold:
            kept.append(_out(f, p))
        else:
            dropped.append(_out(f, p, "below-threshold"))

    verdict = "unjudged" if reason else ("fail" if kept else "pass")
    return report(tool, kept, model, reason, verdict=verdict, dropped=dropped)
```

The `dropped` list is not in input order: fabricated findings come first, then below-threshold ones. `test_keeps_actionable_drops_weak_and_fabricated` expects exactly that order.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd judge && rtk test uv run pytest`
Expected: all pass.

- [ ] **Step 5: Lint, stage, hand off the commit**

```bash
uvx ruff check judge && uvx ruff format judge
rtk git add judge/src/judge/excerpt.py judge/src/judge/filter.py judge/src/judge/schemas judge/tests/test_excerpt.py judge/tests/test_filter.py
cat > .task/commit-msg.txt <<'EOF'
feat(judge): finding schema, disk excerpts and the Jev finding filter

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 7: doc-review agent, runner and the CLI

**Files:**
- Create: `agents/doc-review/doc-review.md`
- Create: `judge/src/judge/doc_review.py`, `judge/src/judge/cli.py`
- Test: `judge/tests/test_doc_review.py`, `judge/tests/test_cli.py`

**Interfaces:**
- Consumes: `filter_.parse_findings`, `filter_.run` (Task 6); `rfc_ready.run` (Task 5); `select_backend`, `JudgeError` (Task 2); `exit_code`, `to_text` (Task 2).
- Produces: `doc_review.run(root, base, whole, budget, backend, claude="claude") -> dict`; `doc_review.ALLOWED_TOOLS`; `cli.main(argv=None) -> int`; `cli.entry()` (console script).
- CLI: `judge rfc-ready [FILE...] [--root DIR] [--format json|text]`; `judge doc-review [--base REF | --all] [--budget USD] [--root DIR] [--format …]`; `judge filter --in FILE [--root DIR] [--format …]`.

- [ ] **Step 1: Write the agent**

`agents/doc-review/doc-review.md`:

```markdown
---
name: doc-review
description: >
  Read-only documentation reviewer. Finds behaviour changed without a doc
  update, docs that contradict the code, and gaps in the house rules the repo
  itself states. Use for "is this change documented?"; `judge doc-review` runs
  it headlessly and filters what it finds.
tools: Read, Grep, Glob, Bash
model: sonnet
---

You review whether a change is properly documented. You never edit files.
Bash is for `git diff`, `git log` and `git ls-files` only.

Start by reading the repo's `CLAUDE.md` and `AGENTS.md`, if present, for its
house rules. Use the rules it states, not rules you assume.

Look for:

1. **missing**: behaviour added or changed without a doc update. That covers
   public functions, CLI flags, config keys, API routes and tasks.
2. **stale** / **wrong**: docs that contradict the code. Wrong flag names,
   removed options, examples that no longer run.
3. House-rule gaps the repo states, such as a task without `desc`, an API
   route without its OpenAPI annotation, or no `docs/` folder.
4. **unclear**: a doc that exists but cannot be followed as written.

Every finding cites a real location: `file` and `line` in the doc (or where
the doc should be), and `code_ref` for the code it is about, or null. Cite only
files `git ls-files` lists. A finding citing anything else is discarded
unread. Report only what a maintainer would change; an empty list is a fine
answer.

Reply with only the JSON object the schema asks for:
`{"findings": [{"file", "line", "kind", "claim", "code_ref", "suggestion"}]}`.
```

- [ ] **Step 2: Write the failing tests**

`judge/tests/test_doc_review.py`:

```python
import json
import os
import subprocess
import sys

import pytest

from judge import doc_review
from judge.backend import FakeBackend, JudgeError

CANNED = {"findings": [
    {"file": "src/thing.py", "line": 1, "kind": "missing", "claim": "max_entries is undocumented",
     "code_ref": {"file": "src/thing.py", "line": 3}, "suggestion": "add it to the readme"},
    {"file": "src/thing.py", "line": 999, "kind": "wrong", "claim": "made up",
     "code_ref": None, "suggestion": ""},
]}


@pytest.fixture
def stub_claude(tmp_path, monkeypatch):
    """A fake `claude` on PATH: records argv and stdin, prints a canned -p JSON result."""
    bindir = tmp_path / "bin"
    bindir.mkdir()
    argv_file = tmp_path / "argv.json"
    out = {"type": "result", "is_error": False, "result": "", "structured_output": CANNED}
    script = bindir / "claude"
    script.write_text(
        f"#!{sys.executable}\nimport json, sys\n"
        f"json.dump({{'argv': sys.argv[1:], 'stdin': sys.stdin.read()}}, open({str(argv_file)!r}, 'w'))\n"
        f"print(json.dumps({out!r}))\n")
    script.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bindir}{os.pathsep}{os.environ['PATH']}")
    return argv_file


def commit(repo):
    subprocess.run(["git", "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "x"],
                   cwd=repo, check=True)


def test_end_to_end_with_stub_claude_and_fake_jev(repo, stub_claude):
    commit(repo)
    backend = FakeBackend({"answers": {"f0": {"type": "noul", "noul": 0.9}}})
    rep = doc_review.run(repo, "HEAD", False, 1.0, backend)
    assert rep["tool"] == "doc-review" and rep["verdict"] == "fail"
    assert [x["message"] for x in rep["items"]] == ["max_entries is undocumented"]
    assert rep["dropped"][0]["dropped_because"] == "fabricated-ref"
    seen = json.loads(stub_claude.read_text())
    argv = seen["argv"]
    assert argv[:4] == ["-p", "--agent", "doc-review", "--output-format"]
    schema = argv[argv.index("--json-schema") + 1]
    assert json.loads(schema)["required"] == ["findings"]  # the text, not a path
    assert argv[argv.index("--max-budget-usd") + 1] == "1.00"
    assert argv[-1] == doc_review.ALLOWED_TOOLS  # nothing positional after the variadic flag
    assert "git diff HEAD...HEAD" in seen["stdin"]


def test_all_scopes_the_whole_repo(repo, stub_claude):
    doc_review.run(repo, "origin/main", True, 0.5, FakeBackend({"answers": {"f0": {"type": "noul", "noul": 0.9}}}))
    assert "whole repository" in json.loads(stub_claude.read_text())["stdin"]


@pytest.mark.parametrize("base", ["no-such-ref", "--output=/tmp/x"])
def test_bad_base_fails_before_spawning_claude(repo, stub_claude, base):
    commit(repo)
    with pytest.raises(JudgeError):
        doc_review.run(repo, base, False, 1.0, None)
    assert not stub_claude.exists()


@pytest.mark.parametrize("stdout,code", [
    ("not json", 0),
    (json.dumps({"is_error": True, "result": "budget exceeded"}), 0),
    (json.dumps({"is_error": False, "structured_output": None, "result": "no json here"}), 0),
    (json.dumps({"is_error": False, "structured_output": {"findings": [{"file": "x"}]}}), 0),
    ("", 1),
])
def test_bad_claude_output_is_a_judge_error(repo, tmp_path, monkeypatch, stdout, code):
    bindir = tmp_path / "bin2"
    bindir.mkdir()
    (bindir / "claude").write_text(f"#!/bin/sh\nprintf '%s' '{stdout}'\nexit {code}\n")
    (bindir / "claude").chmod(0o755)
    monkeypatch.setenv("PATH", f"{bindir}{os.pathsep}{os.environ['PATH']}")
    with pytest.raises(JudgeError):
        doc_review.run(repo, "x", True, 1.0, None)


def test_result_text_is_the_fallback_when_structured_output_is_null(repo, tmp_path, monkeypatch):
    bindir = tmp_path / "bin3"
    bindir.mkdir()
    payload = json.dumps({"is_error": False, "structured_output": None,
                          "result": json.dumps({"findings": []})})
    (bindir / "claude").write_text(f"#!/bin/sh\ncat <<'EOF'\n{payload}\nEOF\n")
    (bindir / "claude").chmod(0o755)
    monkeypatch.setenv("PATH", f"{bindir}{os.pathsep}{os.environ['PATH']}")
    assert doc_review.run(repo, "x", True, 1.0, None)["verdict"] == "pass"
```

`judge/tests/test_cli.py`:

```python
import json

import pytest
from conftest import write_rfc

from judge import cli, rfc_ready


@pytest.fixture(autouse=True)
def no_key(monkeypatch):
    monkeypatch.delenv("TYPESAFE_API_KEY", raising=False)


def test_rfc_ready_without_key_is_exit_3(repo, capsys):
    assert cli.main(["rfc-ready", "--root", str(repo)]) == 3
    rep = json.loads(capsys.readouterr().out)
    assert rep["verdict"] == "unjudged" and rep["report_version"] == 1


def test_missing_block_is_exit_1_without_key(repo, ready_text, capsys):
    write_rfc(repo, ready_text.replace("## 8. Supply chain", "## Supply"))
    assert cli.main(["rfc-ready", "--root", str(repo)]) == 1


def test_text_format(repo, capsys):
    cli.main(["rfc-ready", "--root", str(repo), "--format", "text"])
    assert capsys.readouterr().out.startswith("rfc-ready: UNJUDGED")


def test_filter_command(repo, tmp_path, capsys):
    findings = tmp_path / "f.json"
    findings.write_text(json.dumps({"findings": [
        {"file": "src/thing.py", "line": 1, "kind": "missing", "claim": "c",
         "code_ref": None, "suggestion": ""}]}))
    assert cli.main(["filter", "--in", str(findings), "--root", str(repo)]) == 3
    assert json.loads(capsys.readouterr().out)["tool"] == "filter"


def test_judge_error_is_exit_2(repo, capsys):
    assert cli.main(["rfc-ready", str(repo / "nope.md"), "--root", str(repo)]) == 2
    assert "judge:" in capsys.readouterr().err


def test_unexpected_exception_is_exit_2(repo, monkeypatch):
    def boom(*a):
        raise KeyError("bug")
    monkeypatch.setattr(rfc_ready, "run", boom)
    assert cli.main(["rfc-ready", "--root", str(repo)]) == 2


def test_usage_errors_are_exit_2():
    with pytest.raises(SystemExit) as e:
        cli.main(["doc-review", "--base", "x", "--all"])
    assert e.value.code == 2
    with pytest.raises(SystemExit) as e:
        cli.main(["doc-review", "--budget", "0"])
    assert e.value.code == 2
```

- [ ] **Step 3: Run to verify they fail**

Run: `cd judge && rtk test uv run pytest tests/test_doc_review.py tests/test_cli.py`
Expected: FAIL on import errors.

- [ ] **Step 4: Implement**

`judge/src/judge/doc_review.py`:

```python
"""doc-review: the doc-review agent finds candidate gaps, the filter keeps the
ones that hold up. The agent runs headless in the target repo."""

import json
import subprocess
from importlib import resources

from judge import filter as filter_
from judge.backend import JudgeError

# Pre-approved so the agent can read history; in -p mode any other Bash call
# has no one to approve it and is denied.
ALLOWED_TOOLS = "Read Grep Glob Bash(git diff:*) Bash(git log:*) Bash(git ls-files:*)"


def _scope(base: str, whole: bool) -> str:
    if whole:
        return ("Review the documentation of the whole repository (every file `git ls-files` "
                "lists) against its code and house rules.")
    return (f"Review whether the change `git diff {base}...HEAD` in this repository is properly "
            "documented. Report only gaps that change introduces or leaves.")


def _check_base(root, base: str) -> None:
    # Checked before claude runs so a typo costs nothing; a leading `-` would be read as an option.
    ok = not base.startswith("-") and subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"{base}^{{commit}}"],
        cwd=root, capture_output=True).returncode == 0
    if not ok:
        raise JudgeError(f"--base {base!r} is not a commit here (fetch it, or pass --all)")


def _payload(stdout: str):
    try:
        out = json.loads(stdout)
    except json.JSONDecodeError as e:
        raise JudgeError(f"claude did not print JSON: {stdout[:200]!r}") from e
    if not isinstance(out, dict) or out.get("is_error"):
        raise JudgeError(f"claude reported an error: {str(out)[:500]}")
    # structured_output was observed null on 2026-10-16 with the answer in `result`.
    if out.get("structured_output") is not None:
        return out["structured_output"]
    try:
        return json.loads(out.get("result") or "")
    except json.JSONDecodeError as e:
        raise JudgeError("claude output has neither structured_output nor a JSON result") from e


def run(root, base: str, whole: bool, budget: float, backend, claude: str = "claude") -> dict:
    if not whole:
        _check_base(root, base)
    schema = resources.files("judge").joinpath("schemas/findings.schema.json").read_text()
    cmd = [claude, "-p", "--agent", "doc-review", "--output-format", "json",
           "--json-schema", schema, "--max-budget-usd", f"{budget:.2f}",
           "--allowedTools", ALLOWED_TOOLS]
    try:
        # The prompt goes on stdin: --allowedTools is variadic and would swallow a positional.
        proc = subprocess.run(cmd, cwd=root, input=_scope(base, whole), capture_output=True, text=True)
    except FileNotFoundError as e:
        raise JudgeError(f"`{claude}` is not on PATH") from e
    if proc.returncode != 0:
        raise JudgeError(f"claude exited {proc.returncode}: {(proc.stderr or proc.stdout).strip()[-500:]}")
    findings = filter_.parse_findings(_payload(proc.stdout))
    return filter_.run(findings, root, backend, tool="doc-review")
```

`--json-schema` is passed as an argv item, so no shell quoting is involved. The prompt goes on stdin, which `claude -p` reads when no positional prompt is given. Verified 2026-10-16 with claude 2.1.300: `printf … | claude -p --agent X --allowedTools "Read Grep Bash(git diff:*)"` answered the piped prompt.

`judge/src/judge/cli.py`:

```python
"""judge: headless review gates. Exit 0 pass, 1 fail, 2 error, 3 unjudged."""

import argparse
import json
import subprocess
import sys
import traceback
from pathlib import Path

from judge import doc_review, rfc_ready
from judge import filter as filter_
from judge.backend import JudgeError, select_backend
from judge.report import exit_code, to_text


def _budget(text: str) -> float:
    v = float(text)
    if v <= 0:
        raise argparse.ArgumentTypeError("budget must be positive")
    return v


def _parser() -> argparse.ArgumentParser:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--root", type=Path, help="target repo (default: git root of the cwd)")
    common.add_argument("--format", choices=("json", "text"), default="json")
    p = argparse.ArgumentParser(prog="judge", description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("rfc-ready", parents=[common], help="is an RFC ready for sign-off")
    r.add_argument("files", nargs="*", type=Path)
    d = sub.add_parser("doc-review", parents=[common], help="is the change documented")
    scope = d.add_mutually_exclusive_group()
    scope.add_argument("--base", default="origin/main")
    scope.add_argument("--all", dest="whole", action="store_true")
    d.add_argument("--budget", type=_budget, default=1.0, help="max USD for the agent run")
    f = sub.add_parser("filter", parents=[common], help="filter a findings file through Jev")
    f.add_argument("--in", dest="infile", type=Path, required=True)
    return p


def _git_root() -> Path:
    out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    if out.returncode:
        raise JudgeError("not inside a git repo; pass --root")
    return Path(out.stdout.strip())


def _read_json(path: Path):
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as e:
        raise JudgeError(f"cannot read findings from {path}: {e}") from e


def main(argv=None) -> int:
    args = _parser().parse_args(argv)
    try:
        root = (args.root or _git_root()).resolve()
        backend = select_backend()
        if args.cmd == "rfc-ready":
            rep = rfc_ready.run(args.files, root, backend)
        elif args.cmd == "doc-review":
            rep = doc_review.run(root, args.base, args.whole, args.budget, backend)
        else:
            rep = filter_.run(filter_.parse_findings(_read_json(args.infile)), root, backend)
    except JudgeError as e:
        print(f"judge: {e}", file=sys.stderr)
        return 2
    except Exception:  # a crash is exit 2, never 1, so no gate reads a bug as a judgement
        traceback.print_exc()
        return 2
    print(json.dumps(rep, indent=2, ensure_ascii=False) if args.format == "json" else to_text(rep))
    return exit_code(rep)


def entry() -> None:
    sys.exit(main())
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd judge && rtk test uv run pytest`
Expected: all pass. Then smoke-test the installed entry point: `uv run --project judge judge rfc-ready --root .`. It should exit 0, because this repo has no `docs/rfc/` and so the items list is empty.

- [ ] **Step 6: Lint, stage, hand off the commit**

```bash
uvx ruff check judge && uvx ruff format judge
rtk git add agents/doc-review judge/src/judge/doc_review.py judge/src/judge/cli.py judge/tests/test_doc_review.py judge/tests/test_cli.py
cat > .task/commit-msg.txt <<'EOF'
feat(judge): doc-review agent and the judge CLI

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 8: Wiring — items, profile, tasks, lint, audit, docs

**Files:**
- Modify: `vendor.toml` (`[requires]` table and a new `[installers.judge]`)
- Modify: `profiles.toml` (`planning` items; new `review`)
- Create: `.tasks/agents.yaml`
- Modify: `Taskfile.yaml` (`includes`, `lint`, `audit`). Another session added a `release` task, so edit around it.
- Create: `docs/judge.md`
- Modify: `docs/wska.md` (a short note next to the other session's "Using wska in another project" section), `readme.md` (item table, profile table)

**Interfaces:**
- Consumes: Task 1 (`scope = "any"`, `WSKA_REPO`), Task 7 (`judge` CLI, `agents/doc-review`).

- [ ] **Step 1: `vendor.toml`**

Add to `[requires]`:

```toml
judge           = ["uv"]
doc-review      = ["claude", "judge"]
```

Add after `[installers.batlehub]`:

```toml
# The judge CLI (rfc-ready, doc-review), installed machine-wide by uv. scope
# "any": the review profile is usually installed with --scope project, where a
# user-scoped installer would be skipped, leaving doc-review linked but unable
# to run. --editable mirrors the symlink model: edits here are live everywhere.
[installers.judge]
command = 'uv tool install --force --editable "$WSKA_REPO/judge"'
scope   = "any"
```

- [ ] **Step 2: `profiles.toml`**

Change `[profiles.planning]` to `items = ["bmad", "batlehub-rfc", "judge"]`, and append to the comment above it: `judge brings rfc-ready, the batlehub-rfc review mechanised.`. Append:

```toml
[profiles.review]
include = ["base"]
# judge is the CLI; doc-review the agent `judge doc-review` runs. rfc-ready
# needs no agent, so planning carries judge alone.
items = ["judge", "doc-review"]
```

- [ ] **Step 3: Verify wska sees the new items**

Run: `cargo run --quiet -p wska -- list && cargo run --quiet -p wska -- install --profile review --scope project --dry-run --run-installers && cargo run --quiet -p wska -- doctor`
Expected: `list` shows `doc-review` (agent) and `judge` (installer). The dry run prints `link …/.claude/agents/doc-review -> …/agents/doc-review` and `run judge: uv tool install …`. `doctor` shows `judge` and `doc-review` Ok, or `doc-review` Missing `judge` until the next step.

- [ ] **Step 4: Install for real and check the agent loads**

Run: `cargo run --quiet -p wska -- install --profile review --scope project --run-installers && judge --help`
Expected: `judge` is on PATH (`~/.local/bin`). Then:

Run: `claude -p --agent doc-review --model haiku --max-budget-usd 0.05 "Reply with {\"findings\": []} only"`
Expected: no `--agent 'doc-review' not found`. This costs well under $0.01. Add `.claude/agents/` to `.gitignore` under the `.claude/skills/` line, with the comment `# wska links this repo's agents in here`.

- [ ] **Step 5: `.tasks/agents.yaml`**

```yaml
version: "3"

tasks:
  rfc-ready:
    desc: "Judge whether RFCs are ready for sign-off (usage: task agent:rfc-ready [FILE=docs/rfc/0001-x.md]); exit 0 pass, 1 fail, 2 error, 3 unjudged"
    cmds:
      - judge rfc-ready {{.FILE}}

  doc-review:
    desc: "Check the change is documented: doc-review agent, filtered by Jev (usage: task agent:doc-review [BASE=origin/main] [ALL=1])"
    cmds:
      - judge doc-review {{if .ALL}}--all{{else}}--base {{.BASE | default "origin/main"}}{{end}}

  test:
    desc: "Run the judge test suite (no key, network or LLM needed)"
    cmds:
      - uv run --project judge pytest judge/tests
```

- [ ] **Step 6: `Taskfile.yaml`**

Add `agent: ./.tasks/agents.yaml` to `includes`. Append to `lint.cmds`:

```yaml
      - uvx ruff check judge
      - uvx ruff format --check judge
```

Append to `audit.cmds`:

```yaml
      # Python CVEs in judge's locked dependencies; the hashed export lets
      # pip-audit check the exact pins without resolving anything.
      - mkdir -p .task && uv export --project judge --frozen --no-emit-project -o .task/judge-requirements.txt
      - uvx pip-audit --disable-pip -r .task/judge-requirements.txt
```

- [ ] **Step 7: Verify tasks**

Run: `task --list && task agent:test && task lint && task audit`
Expected: `agent:rfc-ready`, `agent:doc-review` and `agent:test` are listed with descriptions. Tests pass. Lint is clean. pip-audit reports no known vulnerabilities. If pip-audit flags a dependency, bump it with `uv lock --upgrade-package <name>` when the fix is released. Otherwise stop and tell the user.

- [ ] **Step 8: Docs**

`docs/judge.md`, with these sections and this content:

```markdown
# judge — headless review gates

`judge` runs review gates that a hook or CI job can act on. Each command
prints one JSON report on stdout (`--format text` for people) and exits:

| Exit | Meaning |
| --- | --- |
| 0 | pass |
| 1 | fail |
| 2 | usage or internal error (bad flag, bad rubric, bad key, crash) |
| 3 | unjudged: a judgement was needed and Jev could not give one |

`unjudged` is never a pass. Without `TYPESAFE_API_KEY`, deterministic checks
still run, and a deterministic failure is still exit 1.

## Install

`task skills:install PROFILE=review` links the `doc-review` agent and runs
`uv tool install --editable "$WSKA_REPO/judge"`. `planning` installs `judge` alone.

## `judge rfc-ready [FILE...] [--root DIR]`

With no file, judges `docs/rfc/*.md` except `Implemented` and `Rejected`.
Deterministic checks: `heading` (§6.P, §7–§11 present; Jev is skipped when one
is missing), `open-questions` (§15 *Still open* empty), `template-comment` (no
HTML comment left), `symbols` (code-like backticked tokens exist in non-markdown
tracked files; never gates). Jev asks: `block-quality` per mandatory block,
`security-enforced`, `negative-tests`, `safe-defaults`, `probed-numbers`.

Item fields: `file`, `status`, `verdict`, `claim_trust` (0–1, informational),
`verdict_confidence` (0–1: the least certain answer that decided the verdict;
1.0 for a deterministic failure; null when unjudged), `findings`.

## `judge doc-review [--base REF | --all] [--budget USD]`

Runs `claude -p --agent doc-review` (default base `origin/main`, budget 1.00),
then the filter. `fail` if any finding is kept.

## `judge filter --in findings.json`

The filter on its own. Each finding's cited lines are re-read from disk
(tracked files inside the repo only; anything else is dropped as
`fabricated-ref`), then one Jev Noul per finding, `actionable`, kept at ≥ the
threshold. Dropped findings are listed under `dropped` with
`dropped_because`.

## Report

<the top-level JSON example from the spec, and the finding fields:
file, line, message, source (`check:<name>`, `jev:<question-id>` or `agent`),
probability; doc-review findings add kind, code_ref, suggestion>

## Question files

`judge/src/judge/questions/<pipeline>.yaml`: `questions` maps an id to `type`
(`choice` | `score` | `noul`), `instructions`, `criteria` (map for choice and
noul `true`/`false`, ordered list for score). Pipeline keys (`thresholds`,
`weights`, `threshold`) sit beside them. Move thresholds here, not in code. A
file that does not build into SDK questions is exit 2, and `task agent:test`
catches it first.

## Data egress

RFC text and doc/code excerpts are sent to Jev (TypeSafe, US-hosted; retention
undocumented). Untracked and ignored files are never read.
```

Write the Report section out in full from the spec. Do not leave the angle-bracket placeholder.

`docs/wska.md`: next to "Using wska in another project", add:

```markdown
### Installer scope and `WSKA_REPO`

An installer's `scope` is `project`, `user` or `any`. `any` runs at whichever
scope `install` is invoked with: for a machine-wide tool a project-scoped item
needs (`judge`, a `uv tool`). Every installer runs with `WSKA_REPO` set to this
repo's root, so its command can reference files here.
```

`readme.md`: add `judge` (installer, "headless review gates: rfc-ready, doc-review — see docs/judge.md") and `doc-review` (agent) to the item table. Add `review` (`base` + `judge`, `doc-review`) to the profile table, and note `planning` now includes `judge`. Keep the other session's Quick start paragraph intact.

- [ ] **Step 9: Stage and hand off the commit**

```bash
rtk git add vendor.toml profiles.toml .gitignore .tasks/agents.yaml Taskfile.yaml docs/judge.md docs/wska.md readme.md
cat > .task/commit-msg.txt <<'EOF'
feat: wire judge and doc-review into the review and planning profiles

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

If the other session's edits to `Taskfile.yaml`, `docs/wska.md` or `readme.md` are still uncommitted, tell the user that staging these files also stages them. The user should commit theirs first.

---

### Task 9: First live runs (the author, with a key)

Manual. The user runs these steps with `TYPESAFE_API_KEY` set. It is the only check of the vendor's claims.

- [ ] **Step 1:** Write a real RFC with the `batlehub-rfc` skill in a scratch repo, or use an existing one from batlehub. Run `TYPESAFE_LOG_LEVEL=debug judge rfc-ready docs/rfc/<file> --format text`. Check the verdict, confidence and model version are plausible.
- [ ] **Step 2:** Save the raw `system_one` responses as fixtures. Add a temporary `print(resp.model_dump_json())` in `JevBackend.ask`, or copy them from the debug log, into `judge/tests/fixtures/jev/live-*.json` in the fixture format from Task 2. Add one test that replays a live fixture through `FakeBackend` and asserts it parses. Remove the print.
- [ ] **Step 3:** Run `judge doc-review --base origin/main --format text` in this repo. Check that kept findings are real and dropped ones are noise. Tune `threshold` in `finding-filter.yaml` if they are not.
- [ ] **Step 4:** Stage the fixtures and any threshold change. Write the commit message `test(judge): fixtures from the first live Jev runs`.
