"""Parse a batlehub-rfc document and run the deterministic checks. Fenced
code is never a claim: examples are not headings, questions or symbols."""

import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

MANDATORY = ("6.P", "7", "8", "9", "10", "11")
# Blocks are found by heading level and title, the label is only how reports
# name them: the template closes up numbering when optional sections go, so
# §14 may be numbered 13, and a §8 titled "Alternatives" is not Supply chain.
TITLES = {
    "4.1": (3, "configuration"),
    "6.P": (3, "proof"),
    "7": (2, "security"),
    "8": (2, "supply chain"),
    "9": (2, "data"),
    "10": (2, "operability"),
    "10.2": (3, "failure modes"),
    "11": (2, "other concerns"),
    "14": (2, "test plan"),
}

_HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
_NUMBERED = re.compile(r"^(\d+(?:\.(?:\d+|P))?)\.?\s+(.*)$")
_FENCE = re.compile(r"^\s*(```|~~~)")
_ROW = re.compile(r"^\|\s*([^|]+?)\s*\|\s*(.*?)\s*\|\s*$")
_ITEM = re.compile(r"^\s*(?:\d+\.|[-*])\s+\S")
_COMMENT = re.compile(r"<!--.*?(?:-->|\Z)", re.DOTALL)
_TICKED = re.compile(r"`([^`\n]+)`")
_SNAKE = re.compile(r"^[a-z][a-z0-9]*(?:_[a-z0-9]+)+$")
_CAMEL = re.compile(r"^[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]*)+$")
_PATHLIKE = re.compile(r"/.*\.[A-Za-z0-9]+$")


@dataclass
class Section:
    id: str
    title: str
    level: int
    line: int  # 1-based line of the heading
    end: int  # 0-based exclusive end of the body in Rfc.lines
    text: str


@dataclass
class Rfc:
    path: Path
    title: str
    header: dict[str, str]
    sections: dict[str, Section]
    lines: list[str]
    prose: list[str]  # lines with fenced code blanked, same length


def _fence_mask(lines):
    # shortcut: a ```` fence holding a ``` line toggles early; upgrade if RFCs nest fences.
    mask, inside = [], False
    for line in lines:
        if _FENCE.match(line):
            mask.append(True)
            inside = not inside
        else:
            mask.append(inside)
    return mask


def parse(path: Path) -> Rfc:
    lines = path.read_text(encoding="utf-8").splitlines()
    prose = ["" if fenced else line for line, fenced in zip(lines, _fence_mask(lines))]
    headings = []
    for i, line in enumerate(prose):
        m = _HEADING.match(line)
        if m:
            headings.append((i, len(m.group(1)), m.group(2)))
    title = next((t for _, lvl, t in headings if lvl == 1), path.stem)
    sections: dict[str, Section] = {}
    for n, (i, lvl, text) in enumerate(headings):
        m = _NUMBERED.match(text)
        if not m:
            continue
        end = next((j for j, l2, _ in headings[n + 1 :] if l2 <= lvl), len(lines))
        body = "\n".join(lines[i + 1 : end]).strip()
        sections.setdefault(m.group(1), Section(m.group(1), m.group(2), lvl, i + 1, end, body))
    first_section = next((i for i, lvl, _ in headings if lvl == 2), len(lines))
    header = {}
    for line in prose[:first_section]:
        m = _ROW.match(line)
        if m and m.group(1) != "Field" and not set(m.group(1)) <= set("-: "):
            header[m.group(1)] = m.group(2)
    return Rfc(path, title, header, sections, lines, prose)


def _uncommented(rfc: Rfc) -> list[str]:
    text = _COMMENT.sub(lambda m: "\n" * m.group().count("\n"), "\n".join(rfc.prose))
    return text.split("\n")


def prose_text(rfc: Rfc) -> str:
    return "\n".join(rfc.prose).strip()


def section(rfc: Rfc, label: str) -> Section | None:
    level, title = TITLES[label]
    return next(
        (
            s
            for s in rfc.sections.values()
            if s.level == level and s.title.lower().startswith(title)
        ),
        None,
    )


def missing_headings(rfc: Rfc) -> list[str]:
    return [label for label in MANDATORY if section(rfc, label) is None]


def open_questions(rfc: Rfc) -> list[tuple[int, str]]:
    # Found by its heading, wherever the decisions section ended up numbered.
    out, level = [], None
    for i, line in enumerate(_uncommented(rfc)):
        h = _HEADING.match(line)
        if h:
            n = len(h.group(1))
            if h.group(2).strip().lower() == "still open":
                level = n
            elif level is not None and n <= level:
                level = None
        elif level is not None and _ITEM.match(line):
            out.append((i + 1, line.strip()))
    return out


def html_comments(rfc: Rfc) -> list[tuple[int, str]]:
    text = "\n".join(rfc.prose)
    out = []
    for m in _COMMENT.finditer(text):
        inner = m.group().removeprefix("<!--").removesuffix("-->")
        out.append((text.count("\n", 0, m.start()) + 1, " ".join(inner.split())[:80]))
    return out


def looks_like_code(tok: str) -> bool:
    if " " in tok or len(tok) < 3:
        return False
    return (
        "::" in tok
        or "()" in tok
        or bool(_PATHLIKE.search(tok))
        or bool(_SNAKE.match(tok))
        or bool(_CAMEL.match(tok))
    )


def symbols(rfc: Rfc) -> list[tuple[int, str]]:
    seen, out = set(), []
    for i, line in enumerate(_uncommented(rfc)):
        for m in _TICKED.finditer(line):
            tok = m.group(1).strip()
            if looks_like_code(tok) and tok not in seen:
                seen.add(tok)
                out.append((i + 1, tok))
    return out


def symbol_exists(root: Path, tok: str) -> bool:
    if _PATHLIKE.search(tok) and "::" not in tok and (root / tok.lstrip("/")).exists():
        return True
    # Markdown is excluded: the RFC naming a symbol is not evidence it exists.
    # shortcut: `a::b` is grepped verbatim, so `use a::{b}` reads as missing; trust only, never gates.
    needle = tok.removesuffix("()")
    cmd = ["git", "grep", "-F", "-q", "-e", needle, "--", ".", ":(exclude)*.md"]
    return subprocess.run(cmd, check=False, cwd=root, capture_output=True).returncode == 0
