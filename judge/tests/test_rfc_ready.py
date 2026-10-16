import json
from pathlib import Path

import pytest
from conftest import write_rfc

from judge import rfc_ready
from judge.backend import FakeBackend, JudgeError, Unreachable

PASS = json.loads((Path(__file__).parent / "fixtures/jev/rfc-ready-pass.json").read_text())
PASS_TRUST = (1.0 + 0.9 + 0.85 + 0.8 + 0.7) / 5


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
    assert item["claim_trust"] == pytest.approx(PASS_TRUST, abs=1e-3)
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
    filler = {
        "block-quality": {
            "type": "score",
            "score": 0.2,
            "confidence": 0.8,
            "legend": {"0": "p", "1": "b", "2": "r", "3": "s"},
            "probabilities": {"0": 0.85, "1": 0.1, "2": 0.05, "3": 0.0},
        }
    }
    rep, item = one(repo, fake(**{"§7 Security": filler}))
    assert rep["verdict"] == "fail"
    (f,) = item["findings"]
    assert f["source"] == "jev:block-quality"
    assert "§7" in f["message"] and "placeholder" in f["message"]
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
    assert broken["verdict"] == "fail"
    assert broken["findings"][0]["source"] == "check:template-comment"
    assert rep["verdict"] == "fail" and rep["unjudged_reason"] == "no TYPESAFE_API_KEY"


def test_unreachable_is_unjudged_with_reason(repo):
    rep, _item = one(repo, FakeBackend(raises=Unreachable("Jev unreachable: 529")))
    assert rep["verdict"] == "unjudged" and "529" in rep["unjudged_reason"]


def test_missing_symbols_are_findings_but_never_gate(repo, ready_text):
    text = ready_text.replace("Evict cold entries;", "Evict `ghost_symbol` cold entries;")
    p = write_rfc(repo, text)
    rep, item = one(repo, fake(), p.relative_to(repo))
    assert rep["verdict"] == "pass"
    assert [f["source"] for f in item["findings"]] == ["check:symbols"]
    assert item["claim_trust"] < PASS_TRUST


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


def test_discover_skips_real_world_statuses_and_non_rfcs(repo, ready_text):
    write_rfc(repo, ready_text.replace("In review", "**Implemented** — all phases landed"), "2.md")
    write_rfc(repo, ready_text.replace("In review", "**Superseded by 0015** — see it"), "3.md")
    write_rfc(repo, "# RFC index\n\n- [0001](0001-cache-eviction.md)\n", "index.md")
    assert [p.name for p in rfc_ready.discover(repo)] == ["0001-cache-eviction.md"]


def test_states_use_blocks_found_by_title_after_renumbering(repo, ready_text):
    text = ready_text.replace("## 14. Test plan", "## 13. Test plan")
    p = write_rfc(repo, text)
    backend = fake()
    one(repo, backend, p.relative_to(repo))
    (state,) = [s for s, qs in backend.calls if qs == ["negative-tests", "security-enforced"]]
    assert "negative test evicting" in state["text"]
