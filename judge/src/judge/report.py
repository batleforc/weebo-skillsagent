"""The report and exit-code contract every judge command shares."""

SEVERITY = ("pass", "unjudged", "fail")
EXIT = {"pass": 0, "fail": 1, "unjudged": 3}


def finding(file, message, source, line=None, probability=None, **extra) -> dict:
    return {
        "file": file,
        "line": line,
        "message": message,
        "source": source,
        "probability": probability,
        **extra,
    }


def worst(verdicts) -> str:
    return max(verdicts, key=SEVERITY.index, default="pass")


def report(tool, items, judge_model, unjudged_reason, verdict=None, **extra) -> dict:
    return {
        "tool": tool,
        "report_version": 1,
        "verdict": verdict or worst(i["verdict"] for i in items),
        "judge_model": judge_model,
        "unjudged_reason": unjudged_reason,
        "items": items,
        **extra,
    }


def exit_code(rep) -> int:
    return EXIT[rep["verdict"]]


def to_text(rep) -> str:
    head = f"{rep['tool']}: {rep['verdict'].upper()}"
    if rep["judge_model"]:
        head += f" (judge {rep['judge_model']})"
    if rep["unjudged_reason"]:
        head += f" — unjudged: {rep['unjudged_reason']}"
    out = [head]
    for item in rep["items"]:
        if "findings" in item:
            out.append(
                f"  {item['file']}  {item['verdict']}  trust={_num(item['claim_trust'])}"
                f"  confidence={_num(item['verdict_confidence'])}"
            )
            out += [f"    {_line(f)}" for f in item["findings"]]
        else:
            out.append(f"  {_line(item)}")
    out += [f"  dropped ({f['dropped_because']}): {_line(f)}" for f in rep.get("dropped", [])]
    return "\n".join(out)


def _line(f) -> str:
    loc = f"{f['file']}:{f['line']}" if f["line"] else f["file"]
    p = f" p={f['probability']}" if f["probability"] is not None else ""
    return f"{f['source']}  {loc}  {f['message']}{p}"


def _num(v) -> str:
    return "-" if v is None else f"{v:.2f}"
