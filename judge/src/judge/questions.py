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
    return sorted(
        p.name.removesuffix(".yaml") for p in _dir().iterdir() if p.name.endswith(".yaml")
    )


def load(name: str) -> QuestionFile:
    return parse(_dir().joinpath(f"{name}.yaml").read_text(encoding="utf-8"), f"{name}.yaml")


def parse(text: str, origin: str) -> QuestionFile:
    try:
        data = yaml.safe_load(text)
    except yaml.YAMLError as e:
        raise JudgeError(f"{origin}: {e}") from e
    if not isinstance(data, dict) or not isinstance(data.get("questions"), dict):
        raise JudgeError(f"{origin}: needs a non-empty `questions` map")
    if not data["questions"]:
        raise JudgeError(f"{origin}: needs a non-empty `questions` map")
    out = {}
    for qid, spec in data["questions"].items():
        kind = spec.get("type") if isinstance(spec, dict) else None
        cls = _TYPES.get(kind)
        if cls is None:
            raise JudgeError(
                f"{origin}: question {qid}: type must be choice, score or noul, got {kind!r}"
            )
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
