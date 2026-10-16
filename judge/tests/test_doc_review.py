import json
import os
import subprocess
import sys

import pytest

from judge import doc_review
from judge.backend import FakeBackend, JudgeError

CANNED = {
    "findings": [
        {
            "file": "src/thing.py",
            "line": 1,
            "kind": "missing",
            "claim": "max_entries is undocumented",
            "code_ref": {"file": "src/thing.py", "line": 3},
            "suggestion": "add it to the readme",
        },
        {
            "file": "src/thing.py",
            "line": 999,
            "kind": "wrong",
            "claim": "made up",
            "code_ref": None,
            "suggestion": "",
        },
    ]
}
KEPT = FakeBackend({"answers": {"f0": {"type": "noul", "noul": 0.9}}})


@pytest.fixture
def stub_claude(tmp_path, monkeypatch):
    """A fake `claude` on PATH: records argv and stdin, prints a canned -p JSON result."""
    bindir = tmp_path / "bin"
    bindir.mkdir()
    seen = tmp_path / "argv.json"
    out = {"type": "result", "is_error": False, "result": "", "structured_output": CANNED}
    script = bindir / "claude"
    script.write_text(
        f"#!{sys.executable}\nimport json, sys\n"
        f"json.dump({{'argv': sys.argv[1:], 'stdin': sys.stdin.read()}}, open({str(seen)!r}, 'w'))\n"
        f"print(json.dumps({out!r}))\n"
    )
    script.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bindir}{os.pathsep}{os.environ['PATH']}")
    return seen


def fake_claude(tmp_path, monkeypatch, body):
    bindir = tmp_path / "bin2"
    bindir.mkdir()
    (bindir / "claude").write_text("#!/bin/sh\n" + body)
    (bindir / "claude").chmod(0o755)
    monkeypatch.setenv("PATH", f"{bindir}{os.pathsep}{os.environ['PATH']}")


def commit(repo):
    subprocess.run(
        ["git", "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "x"],
        cwd=repo,
        check=True,
    )


def test_end_to_end_with_stub_claude_and_fake_jev(repo, stub_claude):
    commit(repo)
    rep = doc_review.run(repo, "HEAD", False, 1.0, KEPT)
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
    # Anything not pre-approved is denied, whatever the user's defaultMode says.
    assert argv[argv.index("--permission-mode") + 1] == "dontAsk"
    assert argv[argv.index("--tools") + 1] == "Read,Grep,Glob,Bash"
    assert "git diff HEAD...HEAD" in seen["stdin"]


def test_all_scopes_the_whole_repo(repo, stub_claude):
    doc_review.run(repo, "origin/main", True, 0.5, KEPT)
    assert "whole repository" in json.loads(stub_claude.read_text())["stdin"]


@pytest.mark.parametrize("base", ["no-such-ref", "--output=/tmp/x"])
def test_bad_base_fails_before_spawning_claude(repo, stub_claude, base):
    commit(repo)
    with pytest.raises(JudgeError):
        doc_review.run(repo, base, False, 1.0, None)
    assert not stub_claude.exists()


@pytest.mark.parametrize(
    "stdout,code",
    [
        ("not json", 0),
        (json.dumps({"is_error": True, "result": "budget exceeded"}), 0),
        (json.dumps({"is_error": False, "structured_output": None, "result": "no json"}), 0),
        (json.dumps({"is_error": False, "structured_output": {"findings": [{"file": "x"}]}}), 0),
        ("", 1),
    ],
)
def test_bad_claude_output_is_a_judge_error(repo, tmp_path, monkeypatch, stdout, code):
    fake_claude(tmp_path, monkeypatch, f"printf '%s' '{stdout}'\nexit {code}\n")
    with pytest.raises(JudgeError):
        doc_review.run(repo, "x", True, 1.0, None)


def test_result_text_is_the_fallback_when_structured_output_is_null(repo, tmp_path, monkeypatch):
    payload = json.dumps(
        {"is_error": False, "structured_output": None, "result": json.dumps({"findings": []})}
    )
    fake_claude(tmp_path, monkeypatch, f"cat <<'EOF'\n{payload}\nEOF\n")
    assert doc_review.run(repo, "x", True, 1.0, None)["verdict"] == "pass"


def test_claude_missing_from_path_is_a_judge_error(repo):
    with pytest.raises(JudgeError):
        doc_review.run(repo, "x", True, 1.0, None, claude="/nonexistent/claude")
