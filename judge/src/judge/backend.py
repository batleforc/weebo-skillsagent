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
_TRANSIENT = (
    ts.TypeSafeRateLimitError,
    ts.TypeSafeInternalServerError,
    ts.TypeSafeAPIConnectionError,
)


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
