#!/usr/bin/env python3
"""weebo-techdoc helpers: validate a docs/techdoc tree, merge regenerated pages.

Standard library only: it runs inside arbitrary target projects with plain python3.

  techdoc.py check DIR            exit 0 valid, 1 problems, 2 DIR missing
  techdoc.py merge [--partial] OLD NEW [-o OUT]
                                  write merged page to OUT (default OLD), stale ids on
                                  stderr; exit 2 and write nothing on broken markers
"""

import argparse
import json
import re
import sys
from pathlib import Path

BEGIN_RE = re.compile(r"^<!-- techdoc:begin (\S+) -->$")
END = "<!-- techdoc:end -->"
FENCE_RE = re.compile(r"^( {0,3})(`{3,}|~{3,})(.*)$")
KEY_RE = re.compile(r"^[A-Za-z_][\w-]*\s*:")
MAINTENANCE_HEADER = "| Check | Status | What's there | What's missing | Last run |"
MERMAID_TYPES = (
    "flowchart",
    "graph",
    "sequenceDiagram",
    "classDiagram",
    "erDiagram",
    "stateDiagram",
    "stateDiagram-v2",
    "C4Context",
    "C4Container",
    "C4Component",
    "journey",
    "gitGraph",
    "mindmap",
    "timeline",
    "pie",
    "quadrantChart",
    "requirementDiagram",
    "C4Dynamic",
    "C4Deployment",
    "xychart-beta",
    "sankey-beta",
    "block-beta",
    "packet-beta",
    "kanban",
    "architecture-beta",
)
# Frontmatter keys the skill owns; every other key is the user's.
GENERATED_KEYS = ("title", "description", "techdoc_commit")
# Fumadocs meta.json entries that name no page: separators, links, rest, exclusions.
META_SPECIAL_PREFIXES = ("---", "...", "[", "!")


class MarkerError(ValueError):
    pass


def _fences(lines: list[str]) -> list[tuple[int, int | None, str]]:
    """Fenced code blocks (CommonMark): (open line, close line or None if never closed, info)."""
    out, i = [], 0
    while i < len(lines):
        m = FENCE_RE.match(lines[i].rstrip("\r\n"))
        # a backtick fence's info string may not contain a backtick (that's inline code)
        if not m or (m.group(2)[0] == "`" and "`" in m.group(3)):
            i += 1
            continue
        fence, close = m.group(2), None
        for j in range(i + 1, len(lines)):
            raw = lines[j].rstrip("\r\n")
            t = raw.strip()
            if (
                len(raw) - len(raw.lstrip(" ")) < 4
                and t.startswith(fence)
                and t.strip(fence[0]) == ""
            ):
                close = j
                break
        out.append((i, close, m.group(3).strip()))
        if close is None:
            break
        i = close + 1
    return out


def _fenced(lines: list[str]) -> list[bool]:
    """Per line: is it a fence delimiter or inside a fence."""
    flags = [False] * len(lines)
    for a, b, _ in _fences(lines):
        for k in range(a, len(lines) if b is None else b + 1):
            flags[k] = True
    return flags


def _marker_text(line: str) -> str | None:
    """The stripped line if it could be a marker; None when indented 4+ (an indented code block)."""
    raw = line.rstrip("\r\n")
    if raw.startswith("\t") or len(raw) - len(raw.lstrip(" ")) >= 4:
        return None
    return raw.strip()


def _split_frontmatter(text: str) -> tuple[str, str]:
    """(frontmatter including both --- lines, body); frontmatter is '' if absent.

    A leading '---' only opens frontmatter when what follows looks like YAML keys;
    otherwise it's a horizontal rule. A UTF-8 BOM before it is allowed.
    """
    lines = text.splitlines(keepends=True)
    if lines and lines[0].lstrip("\ufeff").strip() == "---":
        for i in range(1, len(lines)):
            if lines[i].strip() == "---":
                inner = [x for x in lines[1:i] if x.strip()]
                if (
                    inner
                    and KEY_RE.match(inner[0])
                    and all(KEY_RE.match(x) or x[0] in " \t-#" for x in inner)
                ):
                    return "".join(lines[: i + 1]), "".join(lines[i + 1 :])
                break
    return "", text


def _fm_entries(fm: str) -> list[tuple[str | None, str]]:
    """Frontmatter body as (key, raw text including continuation lines) in order."""
    entries: list[tuple[str | None, str]] = []
    for line in fm.splitlines(keepends=True)[1:-1]:
        if KEY_RE.match(line):
            entries.append((line.split(":", 1)[0].strip(), line))
        elif entries:
            entries[-1] = (entries[-1][0], entries[-1][1] + line)
        else:
            entries.append((None, line))
    return entries


def _frontmatter_keys(fm: str) -> dict[str, str]:
    return {k: raw.split(":", 1)[1].strip() for k, raw in _fm_entries(fm) if k}


def _eol(text: str, eol: str) -> str:
    return text.replace("\r\n", "\n").replace("\n", eol) if eol == "\r\n" else text


def _scan(text: str) -> list[tuple[str, str | None, int, int]]:
    """Body segments: ("text", None, start, end) or ("block", id, begin_line, end_line).

    Line indices refer to text.splitlines(keepends=True). Markers inside fences or
    indented code are text. An unclosed fence is an error: it would hide every marker after it.
    """
    lines = text.splitlines(keepends=True)
    for a, b, _ in _fences(lines):
        if b is None:
            raise MarkerError(f"line {a + 1}: code fence never closed")
    fenced = _fenced(lines)
    segs, seen, open_id, open_at, text_from = [], set(), None, 0, 0
    for i, line in enumerate(lines):
        if fenced[i]:
            continue
        s = _marker_text(line)
        if s is None:
            continue
        m = BEGIN_RE.match(s)
        if m:
            if open_id is not None:
                raise MarkerError(
                    f"line {i + 1}: begin '{m.group(1)}' inside block '{open_id}'"
                )
            if m.group(1) in seen:
                raise MarkerError(f"line {i + 1}: duplicate block id '{m.group(1)}'")
            if text_from < i:
                segs.append(("text", None, text_from, i))
            open_id, open_at = m.group(1), i
            seen.add(open_id)
        elif s == END:
            if open_id is None:
                raise MarkerError(f"line {i + 1}: end marker without begin")
            segs.append(("block", open_id, open_at, i))
            open_id, text_from = None, i + 1
    if open_id is not None:
        raise MarkerError(f"line {open_at + 1}: block '{open_id}' never closed")
    if text_from < len(lines):
        segs.append(("text", None, text_from, len(lines)))
    return segs


def parse_blocks(text: str) -> dict[str, str]:
    lines = text.splitlines(keepends=True)
    return {
        bid: "".join(lines[a + 1 : b])
        for kind, bid, a, b in _scan(text)
        if kind == "block"
    }


def _merge_frontmatter(old_fm: str, new_fm: str) -> str:
    """Old keys and order kept; GENERATED_KEYS take new's values; new-only keys appended."""
    if not old_fm:
        return new_fm
    if not new_fm:
        return old_fm
    old_lines = old_fm.splitlines(keepends=True)
    eol = "\r\n" if old_lines[0].endswith("\r\n") else "\n"
    new_entries = {k: raw for k, raw in _fm_entries(new_fm) if k}
    out, seen = [old_lines[0]], set()
    for k, raw in _fm_entries(old_fm):
        seen.add(k)
        out.append(
            _eol(new_entries[k], eol)
            if k in GENERATED_KEYS and k in new_entries
            else raw
        )
    out += [_eol(raw, eol) for k, raw in new_entries.items() if k not in seen]
    return "".join(out + old_lines[-1:])


def merge_page(old: str, new: str, partial: bool = False) -> tuple[str, list[str]]:
    """Regenerate old's marked blocks from new; everything outside markers stays byte-identical.

    Returns (merged text, ids of old blocks that new no longer has; those are kept as-is).
    partial=True means new carries only some blocks on purpose: nothing is reported stale.
    Raises MarkerError if either page has broken markers.
    """
    old_fm, old_body = _split_frontmatter(old)
    new_fm, new_body = _split_frontmatter(new)
    new_blocks = parse_blocks(new_body)
    segs = _scan(old_body)
    old_ids = {bid for kind, bid, _, _ in segs if kind == "block"}
    eol = "\r\n" if "\r\n" in old else "\n"

    # New-only blocks hang after their nearest predecessor in new's order (None = top of body).
    after: dict[str | None, list[str]] = {}
    prev = None
    for bid in new_blocks:
        if bid not in old_ids:
            after.setdefault(prev, []).append(bid)
        prev = bid

    def inserted(key: str | None) -> str:
        out = ""
        for bid in after.get(key, []):
            body = f"<!-- techdoc:begin {bid} -->{eol}{_eol(new_blocks[bid], eol)}{END}{eol}"
            out += (eol + body) if key is not None else (body + eol)
            out += inserted(bid)
        return out

    lines = old_body.splitlines(keepends=True)
    out = [inserted(None)]
    stale = []
    for kind, bid, a, b in segs:
        if kind == "text":
            out.append("".join(lines[a:b]))
            continue
        if bid in new_blocks:
            inner = _eol(new_blocks[bid], eol)
        else:
            if not partial:
                stale.append(bid)
            inner = "".join(lines[a + 1 : b])
        out.append(lines[a] + inner + lines[b] + inserted(bid))
    return _merge_frontmatter(old_fm, new_fm) + "".join(out), stale


def _check_page(path: Path, rel: str) -> list[str]:
    text = path.read_text()
    problems = []
    fm, body = _split_frontmatter(text)
    keys = _frontmatter_keys(fm) if fm else {}
    for k in ("title", "description"):
        if not keys.get(k):
            problems.append(f"{rel}: frontmatter needs a non-empty '{k}'")
    lines = body.splitlines()
    fenced = _fenced(lines)
    for i, line in enumerate(lines):
        if not fenced[i] and line.startswith("# "):
            problems.append(
                f"{rel}: '# ' heading found; the title comes from frontmatter, start at '##'"
            )
            break
    try:
        parse_blocks(text)
    except MarkerError as e:
        problems.append(f"{rel}: {e}")
    for i, line in enumerate(lines):
        t = _marker_text(line)
        if fenced[i] or t is None or "techdoc:" not in t:
            continue
        if not (BEGIN_RE.match(t) or t == END):
            problems.append(
                f"{rel}: malformed techdoc marker, each marker goes on its own line: {t}"
            )
    for a, b, info in _fences(lines):
        if b is None or info.split()[:1] != ["mermaid"]:
            continue
        first, in_config = None, False
        for line in lines[a + 1 : b]:
            s = line.strip()
            if "%%{init" in s:
                problems.append(f"{rel}: mermaid theming (%%{{init}}%%) is not allowed")
            elif first is None and s == "---":
                in_config = not in_config  # leading --- config section, not the type
            elif first is None and s and not in_config and not s.startswith("%%"):
                first = s
                if s.split()[0] not in MERMAID_TYPES:
                    problems.append(
                        f"{rel}: unknown mermaid diagram type '{s.split()[0]}'"
                    )
    if path.name == "maintenance.md" and MAINTENANCE_HEADER not in text:
        problems.append(
            f"{rel}: maintenance table header missing: {MAINTENANCE_HEADER}"
        )
    return problems


def check_tree(root: Path) -> list[str]:
    root = Path(root)
    problems = []
    for p in sorted(root.rglob("*.mdx")):
        problems.append(f"{p.relative_to(root)}: .mdx is not allowed, pages are .md")
    md_dirs = set()
    for p in sorted(root.rglob("*.md")):
        md_dirs.add(p.parent)
        problems += _check_page(p, str(p.relative_to(root)))
    for d in sorted(md_dirs):
        rel = str(d.relative_to(root)) if d != root else "."
        meta = d / "meta.json"
        if not meta.is_file():
            problems.append(f"{rel}: meta.json missing")
            continue
        try:
            pages = json.loads(meta.read_text()).get("pages")
        except (json.JSONDecodeError, AttributeError):
            pages = None
        if not isinstance(pages, list):
            problems.append(f"{rel}/meta.json: needs a list 'pages'")
            continue
        for entry in pages:
            if str(entry).startswith(META_SPECIAL_PREFIXES):
                continue
            if not ((d / f"{entry}.md").is_file() or (d / str(entry)).is_dir()):
                problems.append(
                    f"{rel}/meta.json: entry '{entry}' matches no page or folder"
                )
    return problems


def _read_raw(path: Path) -> str:
    """Read without newline translation so CRLF survives a merge."""
    with open(path, encoding="utf-8", newline="") as f:
        return f.read()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="techdoc.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check", help="validate a docs/techdoc tree")
    c.add_argument("dir", type=Path)
    m = sub.add_parser("merge", help="regenerate OLD's marked blocks from NEW")
    m.add_argument("old", type=Path)
    m.add_argument("new", type=Path)
    m.add_argument("-o", "--out", type=Path)
    m.add_argument(
        "--partial",
        action="store_true",
        help="NEW carries only the blocks to add or update; don't report the rest as stale",
    )
    args = ap.parse_args(argv)

    if args.cmd == "check":
        if not args.dir.is_dir():
            print(f"no such directory: {args.dir}", file=sys.stderr)
            return 2
        problems = check_tree(args.dir)
        for p in problems:
            print(p)
        return 1 if problems else 0
    if args.cmd == "merge":
        try:
            merged, stale = merge_page(
                _read_raw(args.old), _read_raw(args.new), partial=args.partial
            )
        except (MarkerError, OSError) as e:
            print(f"merge refused: {e}", file=sys.stderr)
            return 2
        with open(args.out or args.old, "w", encoding="utf-8", newline="") as f:
            f.write(merged)
        for bid in stale:
            print(f"stale: {bid}", file=sys.stderr)
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main())
