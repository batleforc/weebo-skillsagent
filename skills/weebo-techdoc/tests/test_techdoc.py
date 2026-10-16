import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parent.parent / "scripts" / "techdoc.py"
_spec = importlib.util.spec_from_file_location("techdoc", SCRIPT)
techdoc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(techdoc)

FM = "---\ntitle: T\ndescription: D\n---\n"
HEADER = "| Check | Status | What's there | What's missing | Last run |"


def page(body: str = "## Overview\n") -> str:
    return FM + body


def block(bid: str, inner: str) -> str:
    return f"<!-- techdoc:begin {bid} -->\n{inner}<!-- techdoc:end -->\n"


def write(root: Path, rel: str, text: str) -> None:
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text)


def valid_tree(root: Path) -> None:
    write(root, "meta.json", json.dumps({"pages": ["index", "components"]}))
    write(
        root,
        "index.md",
        page(
            "## Overview\n"
            + block("index#architecture", "```mermaid\nflowchart LR\n  a --> b\n```\n")
        ),
    )
    write(root, "components/meta.json", json.dumps({"pages": ["api"]}))
    write(root, "components/api.md", page())


def one_problem(root: Path, needle: str) -> None:
    problems = techdoc.check_tree(root)
    assert len(problems) == 1, problems
    assert needle in problems[0]


# ---- check -------------------------------------------------------------


def test_valid_tree_has_no_problems(tmp_path):
    valid_tree(tmp_path)
    assert techdoc.check_tree(tmp_path) == []


def test_mdx_rejected(tmp_path):
    valid_tree(tmp_path)
    write(tmp_path, "components/x.mdx", page())
    one_problem(tmp_path, "x.mdx")


def test_missing_description(tmp_path):
    valid_tree(tmp_path)
    write(tmp_path, "components/api.md", "---\ntitle: T\n---\n## A\n")
    one_problem(tmp_path, "api.md")


def test_h1_rejected(tmp_path):
    valid_tree(tmp_path)
    write(
        tmp_path, "components/api.md", page("```sh\n# a shell comment\n```\n# Title\n")
    )
    one_problem(tmp_path, "api.md")


def test_unknown_mermaid_type(tmp_path):
    valid_tree(tmp_path)
    write(tmp_path, "components/api.md", page("```mermaid\npie2 title x\n```\n"))
    one_problem(tmp_path, "api.md")


def test_mermaid_init_rejected(tmp_path):
    valid_tree(tmp_path)
    write(
        tmp_path,
        "components/api.md",
        page("```mermaid\n%%{init: {}}%%\nflowchart LR\n  a\n```\n"),
    )
    one_problem(tmp_path, "api.md")


def test_meta_json_missing(tmp_path):
    valid_tree(tmp_path)
    (tmp_path / "components/meta.json").unlink()
    one_problem(tmp_path, "components")


def test_meta_json_dangling_entry(tmp_path):
    valid_tree(tmp_path)
    write(tmp_path, "components/meta.json", json.dumps({"pages": ["api", "ghost"]}))
    one_problem(tmp_path, "ghost")


def test_maintenance_header_required(tmp_path):
    valid_tree(tmp_path)
    write(tmp_path, "maintenance.md", page("| a | b |\n|---|---|\n"))
    write(
        tmp_path,
        "meta.json",
        json.dumps({"pages": ["index", "components", "maintenance"]}),
    )
    one_problem(tmp_path, "maintenance.md")
    write(tmp_path, "maintenance.md", page(HEADER + "\n"))
    assert techdoc.check_tree(tmp_path) == []


def test_marker_error_reported(tmp_path):
    valid_tree(tmp_path)
    write(tmp_path, "components/api.md", page("<!-- techdoc:begin api#role -->\nx\n"))
    one_problem(tmp_path, "api.md")


def test_parse_blocks_ok():
    text = page(block("index#a", "x\n") + "mine\n" + block("index#b", "y\n"))
    assert techdoc.parse_blocks(text) == {"index#a": "x\n", "index#b": "y\n"}


@pytest.mark.parametrize(
    "body",
    [
        "<!-- techdoc:begin a#b -->\nx\n",
        "x\n<!-- techdoc:end -->\n",
        "<!-- techdoc:begin a#b -->\n<!-- techdoc:begin a#c -->\n<!-- techdoc:end -->\n",
        block("a#b", "x\n") + block("a#b", "y\n"),
    ],
    ids=["unclosed", "stray-end", "nested", "duplicate"],
)
def test_parse_blocks_errors(body):
    with pytest.raises(techdoc.MarkerError):
        techdoc.parse_blocks(page(body))


def test_markers_inside_fence_are_ignored():
    assert techdoc.parse_blocks(page("```md\n<!-- techdoc:begin a#b -->\n```\n")) == {}


def test_cli_check_exit_codes(tmp_path):
    def run(d):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "check", str(d)],
            check=False,
            capture_output=True,
        ).returncode

    valid_tree(tmp_path)
    assert run(tmp_path) == 0
    write(tmp_path, "components/x.mdx", page())
    assert run(tmp_path) == 1
    assert run(tmp_path / "nope") == 2


# ---- merge -------------------------------------------------------------


def test_text_outside_markers_is_byte_identical():
    old = (
        FM
        + "intro\n"
        + block("p#a", "old a\n")
        + "between\n"
        + block("p#b", "old b\n")
        + "outro"
    )
    new = FM + block("p#a", "new a\n") + block("p#b", "new b\n")
    merged, stale = techdoc.merge_page(old, new)
    assert merged == old.replace("old a", "new a").replace("old b", "new b")
    assert stale == []


def test_new_block_inserted_after_predecessor():
    old = FM + block("p#a", "a\n") + "mine\n" + block("p#b", "b\n")
    new = FM + block("p#a", "a\n") + block("p#c", "c\n") + block("p#b", "b\n")
    merged, _ = techdoc.merge_page(old, new)
    assert merged == FM + block("p#a", "a\n") + "\n" + block(
        "p#c", "c\n"
    ) + "mine\n" + block("p#b", "b\n")


def test_new_block_without_predecessor_goes_after_frontmatter():
    old = FM + "mine\n" + block("p#b", "b\n")
    new = FM + block("p#first", "f\n") + block("p#b", "b\n")
    merged, _ = techdoc.merge_page(old, new)
    assert merged == FM + block("p#first", "f\n") + "\n" + "mine\n" + block(
        "p#b", "b\n"
    )


def test_missing_block_kept_and_reported():
    old = FM + block("p#a", "a\n") + block("p#b", "b\n")
    new = FM + block("p#a", "A\n")
    merged, stale = techdoc.merge_page(old, new)
    assert merged == FM + block("p#a", "A\n") + block("p#b", "b\n")
    assert stale == ["p#b"]


def test_fenced_marker_is_user_text():
    example = "```md\n<!-- techdoc:begin x#y -->\nexample\n<!-- techdoc:end -->\n```\n"
    old = FM + example + block("p#a", "a\n")
    new = FM + block("p#a", "A\n")
    merged, _ = techdoc.merge_page(old, new)
    assert merged == FM + example + block("p#a", "A\n")


def test_unbalanced_old_refuses():
    with pytest.raises(techdoc.MarkerError):
        techdoc.merge_page(
            FM + "<!-- techdoc:begin p#a -->\nx\n", FM + block("p#a", "a\n")
        )


def test_frontmatter_keeps_user_keys():
    old = "---\ntitle: Old\nicon: Server\n---\nbody\n"
    new = "---\ntitle: New\ndescription: D\n---\n"
    merged, _ = techdoc.merge_page(old, new)
    assert merged == "---\ntitle: New\nicon: Server\ndescription: D\n---\nbody\n"


def test_crlf_and_no_trailing_newline_preserved():
    old = (
        "---\r\ntitle: T\r\ndescription: D\r\n---\r\nmine\r\n"
        + block("p#a", "a\n").replace("\n", "\r\n")
        + "tail"
    )
    new = FM + block("p#a", "A\n")
    merged, _ = techdoc.merge_page(old, new)
    assert merged.startswith(
        "---\r\ntitle: T\r\ndescription: D\r\n---\r\nmine\r\n<!-- techdoc:begin p#a -->\r\n"
    )
    assert merged.endswith("<!-- techdoc:end -->\r\ntail")
    assert "A\r\n" in merged


def test_cli_merge_writes_and_reports_stale(tmp_path):
    o, n = tmp_path / "old.md", tmp_path / "new.md"
    o.write_text(FM + block("p#a", "a\n") + block("p#b", "b\n"))
    n.write_text(FM + block("p#a", "A\n"))
    r = subprocess.run(
        [sys.executable, str(SCRIPT), "merge", str(o), str(n)],
        check=False,
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0
    assert "stale: p#b" in r.stderr
    assert "A\n" in o.read_text()


def test_cli_merge_refuse_leaves_file_untouched(tmp_path):
    o, n = tmp_path / "old.md", tmp_path / "new.md"
    bad = FM + "<!-- techdoc:begin p#a -->\nx\n"
    o.write_text(bad)
    n.write_text(FM + block("p#a", "A\n"))
    r = subprocess.run(
        [sys.executable, str(SCRIPT), "merge", str(o), str(n)],
        check=False,
        capture_output=True,
    )
    assert r.returncode == 2
    assert o.read_text() == bad


# ---- final review fixes ----------------------------------------------


def test_long_fence_not_closed_by_shorter():
    example = "````md\n```\n<!-- techdoc:begin p#a -->\nexample\n<!-- techdoc:end -->\n```\n````\n"
    old = FM + example + block("p#a", "a\n")
    merged, stale = techdoc.merge_page(old, FM + block("p#a", "NEW\n"))
    assert merged == FM + example + block("p#a", "NEW\n")
    assert stale == []


def test_frontmatter_takes_techdoc_commit():
    old = "---\ntitle: T\ndescription: D\ntechdoc_commit: aaa\n---\n"
    new = "---\ntitle: T\ndescription: D\ntechdoc_commit: bbb\n---\n"
    merged, _ = techdoc.merge_page(old, new)
    assert "techdoc_commit: bbb" in merged
    assert "aaa" not in merged


def test_meta_json_fumadocs_special_entries_ok(tmp_path):
    valid_tree(tmp_path)
    write(
        tmp_path,
        "meta.json",
        json.dumps(
            {
                "pages": [
                    "index",
                    "---Guides---",
                    "[IDP](https://x)",
                    "...",
                    "!hidden",
                    "components",
                ]
            }
        ),
    )
    assert techdoc.check_tree(tmp_path) == []


@pytest.mark.parametrize(
    "diagram",
    [
        "%% a comment\nflowchart LR\n  a --> b\n",
        "---\ntitle: Items\n---\nflowchart LR\n  a --> b\n",
        '\npie title x\n  "a" : 1\n',
        "architecture-beta\n  service a(server)[A]\n",
        "C4Dynamic\n",
    ],
    ids=["comment", "config", "pie", "architecture-beta", "c4dynamic"],
)
def test_mermaid_valid_variants_ok(tmp_path, diagram):
    valid_tree(tmp_path)
    write(tmp_path, "components/api.md", page("```mermaid\n" + diagram + "```\n"))
    assert techdoc.check_tree(tmp_path) == []


# ---- deferred minors ---------------------------------------------------


def test_mermaid_inside_outer_fence_not_checked(tmp_path):
    valid_tree(tmp_path)
    example = "````md\n```mermaid\n%%{init: {}}%%\npie2\n```\n````\n"
    write(tmp_path, "components/api.md", page(example))
    assert techdoc.check_tree(tmp_path) == []


@pytest.mark.parametrize("fence", ["~~~", "````"])
def test_tilde_and_long_mermaid_fences_checked(tmp_path, fence):
    valid_tree(tmp_path)
    write(tmp_path, "components/api.md", page(f"{fence}mermaid\npie2 x\n{fence}\n"))
    one_problem(tmp_path, "api.md")


def test_indented_marker_is_code():
    body = "    <!-- techdoc:begin p#a -->\n\n" + block("p#a", "x\n")
    assert techdoc.parse_blocks(page(body)) == {"p#a": "x\n"}


def test_unclosed_fence_refused():
    with pytest.raises(techdoc.MarkerError):
        techdoc.parse_blocks(
            page(block("p#a", "a\n") + "```\nunclosed\n" + block("p#b", "b\n"))
        )


def test_backtick_info_string_is_not_a_fence():
    body = "``` js `x`\n" + block("p#a", "x\n")
    assert techdoc.parse_blocks(page(body)) == {"p#a": "x\n"}


def test_multiline_frontmatter_value_replaced():
    old = "---\ntitle: T\ndescription: >\n  long old\n  desc\nicon: X\n---\nbody\n"
    new = "---\ntitle: T\ndescription: >\n  new\n  desc\n---\n"
    merged, _ = techdoc.merge_page(old, new)
    assert (
        merged == "---\ntitle: T\ndescription: >\n  new\n  desc\nicon: X\n---\nbody\n"
    )


def test_horizontal_rule_is_not_frontmatter():
    old = "---\n" + block("p#a", "o\n") + "\n---\nuser\n"
    merged, _ = techdoc.merge_page(old, FM + block("p#a", "N\n"))
    assert merged == FM + old.replace("o\n", "N\n")


def test_bom_frontmatter_stays_single():
    old = "﻿" + FM + block("p#a", "a\n")
    merged, _ = techdoc.merge_page(old, FM + block("p#a", "A\n"))
    assert merged == "﻿" + FM + block("p#a", "A\n")


def test_crlf_page_gets_crlf_generated_text():
    old = (FM + block("p#a", "a\n")).replace("\n", "\r\n")
    new = FM + block("p#a", "A\nB\n") + block("p#c", "C\n")
    merged, _ = techdoc.merge_page(old, new)
    assert re.search(r"(?<!\r)\n", merged) is None
    assert "A\r\nB\r\n" in merged and "C\r\n" in merged


def test_partial_merge_updates_only_given_blocks():
    notice = block("p#notice", "n\n")
    old = FM + notice + block("p#a", "a\n") + "mine\n" + block("p#b", "b\n")
    new = FM + notice + block("p#orphaned", "gone\n")
    merged, stale = techdoc.merge_page(old, new, partial=True)
    assert stale == []
    assert merged == FM + notice + "\n" + block("p#orphaned", "gone\n") + block(
        "p#a", "a\n"
    ) + "mine\n" + block("p#b", "b\n")


def test_cli_merge_partial(tmp_path):
    o, n = tmp_path / "old.md", tmp_path / "new.md"
    o.write_text(FM + block("p#a", "a\n") + block("p#b", "b\n"))
    n.write_text(FM + block("p#a", "A\n"))
    r = subprocess.run(
        [sys.executable, str(SCRIPT), "merge", "--partial", str(o), str(n)],
        check=False,
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0
    assert "stale" not in r.stderr
    assert o.read_text() == FM + block("p#a", "A\n") + block("p#b", "b\n")


def test_malformed_marker_line_flagged(tmp_path):
    valid_tree(tmp_path)
    write(
        tmp_path,
        "components/api.md",
        page("<!-- techdoc:begin components/api#notice -->...<!-- techdoc:end -->\n"),
    )
    one_problem(tmp_path, "api.md")
