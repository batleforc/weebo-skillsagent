import json
from importlib import resources

import pytest

from judge import filter as filter_
from judge.backend import FakeBackend, JudgeError, Unreachable


def f(file="src/thing.py", line=1, ref=None, kind="missing"):
    return {
        "file": file,
        "line": line,
        "kind": kind,
        "claim": "undocumented",
        "code_ref": ref,
        "suggestion": "document it",
    }


def yes(p):
    return {"type": "noul", "noul": p}


def test_schema_and_validator_agree_on_kinds():
    text = resources.files("judge").joinpath("schemas/findings.schema.json").read_text()
    kinds = json.loads(text)["properties"]["findings"]["items"]["properties"]["kind"]["enum"]
    assert tuple(kinds) == filter_.KINDS


@pytest.mark.parametrize(
    "obj",
    [
        [],
        {"findings": "x"},
        {"findings": [{"file": "a"}]},
        {"findings": [f(kind="other")]},
        {"findings": [f(line=0)]},
        {"findings": [f(line=True)]},
        {"findings": [f(ref={"file": ""})]},
    ],
)
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
        ("fabricated-ref", None),
        ("below-threshold", 0.3),
    ]
    (call,) = backend.calls  # one batched request
    assert call[1] == ["f0", "f2"]


def test_state_holds_real_excerpts_not_agent_quotes(repo):
    backend = FakeBackend({"answers": {"f0": yes(0.9)}})
    filter_.run([f() | {"claim": "the file says HELLO"}], repo, backend)
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
