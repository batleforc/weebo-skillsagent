"""rfc-ready: is an RFC ready for sign-off, how far can its claims be trusted,
and how sure is that verdict. Deterministic checks first; Jev only for what
needs judgement. The rules are batlehub-rfc's §4 and §7; this mechanises them."""

import re
from pathlib import Path

from judge import questions
from judge import rfc as rfcmod
from judge.backend import JudgeError, Unreachable
from judge.report import finding, report

LEVELS = ("placeholder", "bare none", "reasoned none", "substantive")  # rfc-ready.yaml order
READY = LEVELS.index("reasoned none")
GATES = ("security-enforced", "negative-tests", "safe-defaults")
TRUST = ("security-enforced", "negative-tests", "safe-defaults", "probed-numbers")
# Real Status cells read "**Implemented** — all phases landed", "Superseded by 0015".
SKIP = re.compile(r"[\W_]*(implemented|rejected|superseded)\b", re.IGNORECASE)
MESSAGES = {
    "security-enforced": "a security claim in §7 names no code that enforces it",
    "negative-tests": "a threat in §7 has no negative test in §14",
    "safe-defaults": (
        "a new default or control in §4.1/§10.2 is not the safe side,"
        " and the text does not argue why"
    ),
}


def _parse(path: Path) -> rfcmod.Rfc:
    if not path.is_file():
        raise JudgeError(f"no such RFC: {path}")
    try:
        return rfcmod.parse(path)
    except (OSError, UnicodeDecodeError) as e:
        raise JudgeError(f"cannot read {path}: {e}") from e


def discover(root: Path) -> list[Path]:
    """RFCs still in play. A file with no Status row (an index, a plan) is not an RFC."""
    out = []
    for p in sorted((root / "docs" / "rfc").glob("*.md")):
        status = _parse(p).header.get("Status")
        if status is not None and not SKIP.match(status):
            out.append(p)
    return out


def run(files, root: Path, backend) -> dict:
    qf = questions.load("rfc-ready")
    items, reasons = [], []
    for path in [Path(f) for f in files] or discover(root):
        item, reason = judge_one(_parse(path), root, backend, qf)
        items.append(item)
        if reason and reason not in reasons:
            reasons.append(reason)
    model = backend.model if backend else None
    return report("rfc-ready", items, model, "; ".join(reasons) or None)


def _rel(path: Path, root: Path) -> str:
    path, root = path.resolve(), root.resolve()
    return str(path.relative_to(root)) if path.is_relative_to(root) else str(path)


def _state(doc, part, *labels):
    parts = []
    for label in labels:
        sec = rfcmod.section(doc, label)
        parts.append(
            f"§{sec.id} {sec.title}\n{sec.text}" if sec else f"§{label}: absent from this RFC"
        )
    return {"rfc": doc.title, "part": part, "text": "\n\n".join(parts)}


def _ask(doc, backend, qf):
    q = qf.questions
    bq = {"block-quality": q["block-quality"]}
    blocks = {
        b: backend.ask(_state(doc, f"§{b}", b), bq)["block-quality"] for b in rfcmod.MANDATORY
    }
    whole = {
        "rfc": doc.title,
        "part": "whole RFC, code blocks stripped",
        "text": rfcmod.prose_text(doc),
    }
    nouls = {}
    for state, ids in (
        (_state(doc, "§7 + §14", "7", "14"), ("security-enforced", "negative-tests")),
        (_state(doc, "§4.1 + §10.2", "4.1", "10.2"), ("safe-defaults",)),
        (whole, ("probed-numbers",)),
    ):
        got = backend.ask(state, {i: q[i] for i in ids})
        nouls.update({i: got[i].noul for i in ids})
    return blocks, nouls


def judge_one(doc, root: Path, backend, qf) -> tuple[dict, str | None]:
    rel = _rel(doc.path, root)
    missing = rfcmod.missing_headings(doc)
    findings = [finding(rel, f"mandatory block §{s} is missing", "check:heading") for s in missing]
    findings += [
        finding(rel, f"question still open: {t}", "check:open-questions", line)
        for line, t in rfcmod.open_questions(doc)
    ]
    findings += [
        finding(rel, f"template comment left in: {t}", "check:template-comment", line)
        for line, t in rfcmod.html_comments(doc)
    ]
    deterministic_fail = bool(findings)

    syms = rfcmod.symbols(doc)
    absent = [(line, tok) for line, tok in syms if not rfcmod.symbol_exists(root, tok)]
    findings += [
        finding(rel, f"`{tok}` not found in the repo", "check:symbols", line)
        for line, tok in absent
    ]
    trust = {"symbols": 1 - len(absent) / len(syms)} if syms else {}

    reason, deciding = None, []  # deciding: (passed, certainty) per gating answer
    if missing:
        pass  # a missing block needs no judge
    elif backend is None:
        reason = "no TYPESAFE_API_KEY"
    else:
        try:
            blocks, nouls = _ask(doc, backend, qf)
        except Unreachable as e:
            reason = str(e)
        else:
            th = qf.extra.get("thresholds", {})
            for b, a in blocks.items():
                p = sum(v for lvl, v in a.probabilities.items() if lvl >= READY)
                ok = p >= th.get("block-quality", 0.5)
                deciding.append((ok, a.confidence))
                if not ok:
                    level = LEVELS[max(a.probabilities, key=a.probabilities.get)]
                    findings.append(
                        finding(
                            rel,
                            f"§{b} reads as '{level}', below 'reasoned none'",
                            "jev:block-quality",
                            rfcmod.section(doc, b).line,
                            round(p, 3),
                        )
                    )
            for q in GATES:
                ok = nouls[q] >= th.get(q, 0.5)
                deciding.append((ok, 2 * abs(nouls[q] - 0.5)))
                if not ok:
                    findings.append(finding(rel, MESSAGES[q], f"jev:{q}", None, round(nouls[q], 3)))
            trust.update({q: nouls[q] for q in TRUST})

    failed = [c for ok, c in deciding if not ok]
    if deterministic_fail:
        verdict, confidence = "fail", 1.0
    elif reason:
        verdict, confidence = "unjudged", None
    elif failed:
        verdict, confidence = "fail", min(failed)
    else:
        verdict, confidence = "pass", min(c for _, c in deciding)

    weights = qf.extra.get("weights", {})
    total = sum(weights.get(k, 1) for k in trust)
    claim_trust = (
        round(sum(weights.get(k, 1) * v for k, v in trust.items()) / total, 3) if total else None
    )
    item = {
        "file": rel,
        "status": doc.header.get("Status"),
        "verdict": verdict,
        "claim_trust": claim_trust,
        "verdict_confidence": None if confidence is None else round(confidence, 3),
        "findings": findings,
    }
    return item, reason
