"""doc-review: the doc-review agent finds candidate gaps, the filter keeps the
ones that hold up. The agent runs headless in the target repo."""

import json
import subprocess
from importlib import resources

from judge import filter as filter_
from judge.backend import JudgeError

# Pre-approved so the agent can read history. --permission-mode dontAsk denies
# everything else, whatever defaultMode the user's settings carry: the agent
# reads diffs a PR author controls, so prose alone must not limit its Bash.
TOOLS = "Read,Grep,Glob,Bash"
ALLOWED_TOOLS = "Read Grep Glob Bash(git diff:*) Bash(git log:*) Bash(git ls-files:*)"


def _scope(base: str, whole: bool) -> str:
    if whole:
        return (
            "Review the documentation of the whole repository (every file `git ls-files` "
            "lists) against its code and house rules."
        )
    return (
        f"Review whether the change `git diff {base}...HEAD` in this repository is properly "
        "documented. Report only gaps that change introduces or leaves."
    )


def _check_base(root, base: str) -> None:
    # Checked before claude runs so a typo costs nothing; a leading `-` would be read as an option.
    ok = (
        not base.startswith("-")
        and subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", f"{base}^{{commit}}"],
            check=False,
            cwd=root,
            capture_output=True,
        ).returncode
        == 0
    )
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
    cmd = [
        claude, "-p", "--agent", "doc-review", "--output-format", "json",
        "--json-schema", schema, "--max-budget-usd", f"{budget:.2f}",
        "--permission-mode", "dontAsk", "--tools", TOOLS,
        "--allowedTools", ALLOWED_TOOLS,
    ]  # fmt: skip
    try:
        # The prompt goes on stdin: --allowedTools is variadic and would swallow a positional.
        proc = subprocess.run(
            cmd,
            check=False,
            cwd=root,
            input=_scope(base, whole),
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as e:
        raise JudgeError(f"`{claude}` is not on PATH") from e
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout).strip()[-500:]
        raise JudgeError(f"claude exited {proc.returncode}: {detail}")
    findings = filter_.parse_findings(_payload(proc.stdout))
    return filter_.run(findings, root, backend, tool="doc-review")
