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


def test_missing_block_is_exit_1_without_key(repo, ready_text):
    write_rfc(repo, ready_text.replace("## 8. Supply chain", "## Supply"))
    assert cli.main(["rfc-ready", "--root", str(repo)]) == 1


def test_text_format(repo, capsys):
    cli.main(["rfc-ready", "--root", str(repo), "--format", "text"])
    assert capsys.readouterr().out.startswith("rfc-ready: UNJUDGED")


def test_filter_command(repo, tmp_path, capsys):
    findings = tmp_path / "f.json"
    finding = {
        "file": "src/thing.py",
        "line": 1,
        "kind": "missing",
        "claim": "c",
        "code_ref": None,
        "suggestion": "",
    }
    findings.write_text(json.dumps({"findings": [finding]}))
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


def test_a_crash_while_printing_is_exit_2(repo, monkeypatch):
    def boom(rep):
        raise KeyError("bug")

    monkeypatch.setattr(cli, "to_text", boom)
    assert cli.main(["rfc-ready", "--root", str(repo), "--format", "text"]) == 2


def test_a_root_that_is_not_a_directory_is_exit_2(tmp_path, capsys):
    assert cli.main(["rfc-ready", "--root", str(tmp_path / "typo")]) == 2
    assert "not a directory" in capsys.readouterr().err
