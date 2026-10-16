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
        "tool": "rfc-ready",
        "report_version": 1,
        "verdict": "fail",
        "judge_model": "jev-1",
        "unjudged_reason": None,
        "items": [{"verdict": "pass"}, {"verdict": "fail"}],
    }


def test_explicit_verdict_and_extra_keys():
    rep = report("doc-review", [], None, "no key", verdict="unjudged", dropped=[])
    assert rep["verdict"] == "unjudged" and rep["dropped"] == []


def test_finding_carries_the_contract_fields():
    f = finding("a.md", "msg", "check:heading")
    assert f == {
        "file": "a.md",
        "line": None,
        "message": "msg",
        "source": "check:heading",
        "probability": None,
    }


def test_text_format_mentions_verdict_and_findings():
    item = {
        "file": "a.md",
        "verdict": "fail",
        "claim_trust": 0.5,
        "verdict_confidence": 1.0,
        "findings": [finding("a.md", "§9 missing", "check:heading")],
    }
    text = to_text(report("rfc-ready", [item], None, None))
    assert "FAIL" in text and "§9 missing" in text and "check:heading" in text
