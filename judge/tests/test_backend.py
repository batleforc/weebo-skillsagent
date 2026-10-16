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
    fake = FakeBackend(
        {
            "answers": {"q": {"type": "noul", "noul": 0.9}},
            "by_state": [{"contains": "§7", "answers": {"q": {"type": "noul", "noul": 0.1}}}],
        }
    )
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


@pytest.mark.parametrize(
    "exc",
    [
        ts.TypeSafeRateLimitError(429, {}, httpx2.Headers()),
        ts.TypeSafeInternalServerError(529, {}, httpx2.Headers()),
        ts.TypeSafeAPIConnectionError("down"),
    ],
)
def test_transient_errors_are_unreachable(exc):
    with pytest.raises(Unreachable):
        JevBackend(client=_Client(exc=exc)).ask({}, {"q": NOUL})


@pytest.mark.parametrize(
    "exc",
    [
        ts.TypeSafeBadRequestError(400, {}, httpx2.Headers()),
        ts.TypeSafeAuthenticationError(401, {}, httpx2.Headers()),
        ts.TypeSafeUnprocessableEntityError(422, {}, httpx2.Headers()),
        ts.TypeSafeError("bad question"),
    ],
)
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
