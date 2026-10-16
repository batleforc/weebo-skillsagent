"""The finding filter, shared by every reviewer: drop findings that cite
nothing real, then keep only those Jev judges actionable. Dropped findings
stay in the report so the filter itself can be audited."""

from judge import questions
from judge.backend import JudgeError, Unreachable
from judge.excerpt import excerpt
from judge.report import finding, report

KINDS = ("missing", "stale", "wrong", "unclear")  # = findings.schema.json


def _text(v):
    return isinstance(v, str) and v.strip() != ""


def _line(v):
    return v is None or (isinstance(v, int) and not isinstance(v, bool) and v >= 1)


def _valid(f) -> bool:
    if not isinstance(f, dict):
        return False
    ref = f.get("code_ref")
    ref_ok = ref is None or (
        isinstance(ref, dict) and _text(ref.get("file")) and _line(ref.get("line"))
    )
    return (
        _text(f.get("file"))
        and _line(f.get("line"))
        and f.get("kind") in KINDS
        and _text(f.get("claim"))
        and isinstance(f.get("suggestion", ""), str)
        and ref_ok
    )


def parse_findings(obj) -> list[dict]:
    """Validate at the trust boundary: this is LLM output."""
    if not isinstance(obj, dict) or not isinstance(obj.get("findings"), list):
        raise JudgeError("findings must be an object with a `findings` list")
    for n, f in enumerate(obj["findings"]):
        if not _valid(f):
            raise JudgeError(f"finding {n} does not match findings.schema.json: {f!r:.200}")
    return obj["findings"]


def _out(f, p, why=None) -> dict:
    out = finding(
        f["file"],
        f["claim"],
        "jev:actionable" if p is not None else "agent",
        f.get("line"),
        p,
        kind=f["kind"],
        code_ref=f.get("code_ref"),
        suggestion=f.get("suggestion", ""),
    )
    if why:
        out["dropped_because"] = why
    return out


def run(findings, root, backend, tool="filter") -> dict:
    qf = questions.load("finding-filter")
    threshold = qf.extra.get("threshold", 0.6)
    base = qf.questions["actionable"]
    kept, dropped, survivors = [], [], {}
    for n, f in enumerate(findings):
        doc = excerpt(root, f["file"], f.get("line"))
        ref = f.get("code_ref")
        code = excerpt(root, ref["file"], ref.get("line")) if ref else None
        if doc is None or (ref and code is None):
            dropped.append(_out(f, None, "fabricated-ref"))
            continue
        survivors[f"f{n}"] = (
            f,
            {
                "kind": f["kind"],
                "claim": f["claim"],
                "suggestion": f.get("suggestion", ""),
                "doc_excerpt": doc,
                "code_excerpt": code,
            },
        )

    reason, model, got = None, None, {}
    if survivors and backend is None:
        reason = "no TYPESAFE_API_KEY"
    elif survivors:
        state = {"findings": {fid: s for fid, (_, s) in survivors.items()}}
        qs = {
            fid: base.model_copy(update={"instructions": base.instructions.replace("{id}", fid)})
            for fid in survivors
        }
        try:
            got = backend.ask(state, qs)
            model = backend.model
        except Unreachable as e:
            reason = str(e)

    for fid, (f, _) in survivors.items():
        if reason:
            kept.append(_out(f, None))
            continue
        p = round(got[fid].noul, 3)
        if p >= threshold:
            kept.append(_out(f, p))
        else:
            dropped.append(_out(f, p, "below-threshold"))

    verdict = "unjudged" if reason else ("fail" if kept else "pass")
    return report(tool, kept, model, reason, verdict=verdict, dropped=dropped)
