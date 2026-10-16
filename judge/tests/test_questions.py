import pytest
import typesafe_sdk as ts

from judge import questions
from judge.backend import JudgeError


def test_both_rubrics_are_packaged():
    assert questions.names() == ["finding-filter", "rfc-ready"]


@pytest.mark.parametrize("name", ["finding-filter", "rfc-ready"])
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
        "    criteria:\n      true: yes it is\n      false: no it is not\n",
        "t",
    )
    assert qf.questions["q"].criteria == {"true": "yes it is", "false": "no it is not"}


@pytest.mark.parametrize(
    "text",
    [
        "not: [valid",  # YAML error
        "questions: {}",  # empty
        "questions:\n  q:\n    type: box\n",  # unsupported type
        "questions:\n  q:\n    type: score\n    criteria: []\n",
        "questions:\n  q:\n    type: choice\n    criteria: {}\n",
        "questions:\n  q:\n    type: noul\n    bogus: 1\n",  # unknown field
    ],
)
def test_bad_files_are_judge_errors(text):
    with pytest.raises(JudgeError):
        questions.parse(text, "t")
