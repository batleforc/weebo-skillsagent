from conftest import write_rfc

from judge import rfc


def test_header_title_and_sections(repo):
    doc = rfc.parse(repo / "docs/rfc/0001-cache-eviction.md")
    assert doc.title == "RFC 0001 — Cache eviction"
    assert doc.header["Status"] == "In review"
    assert "Field" not in doc.header
    expected = {"1", "4", "4.1", "6", "6.P", "7", "8", "9", "10", "10.2", "11", "14", "15"}
    assert expected <= set(doc.sections)
    assert doc.sections["7"].title == "Security"
    assert "Eviction can only delete" in doc.sections["7"].text
    assert "If eviction fails" in doc.sections["10"].text  # a ## section includes its ### ones


def test_ready_rfc_passes_every_deterministic_check(repo):
    doc = rfc.parse(repo / "docs/rfc/0001-cache-eviction.md")
    assert rfc.missing_headings(doc) == []
    assert rfc.open_questions(doc) == []
    assert rfc.html_comments(doc) == []


def test_missing_mandatory_heading(repo, ready_text):
    doc = rfc.parse(write_rfc(repo, ready_text.replace("## 9. Data and privacy", "## Data")))
    assert rfc.missing_headings(doc) == ["9"]


def test_fenced_heading_and_item_are_ignored(repo, ready_text):
    # The fenced example in §1 holds "## 9. Data…" and "1. an example list item".
    text = ready_text.replace("## 9. Data and privacy\n", "").replace(
        "### Still open\n", "### Still open\n\n```\n1. fenced, not open\n```\n"
    )
    doc = rfc.parse(write_rfc(repo, text))
    assert rfc.missing_headings(doc) == ["9"]
    assert rfc.open_questions(doc) == []


def test_open_questions_are_counted_with_lines(repo, ready_text):
    text = ready_text.replace(
        "### Still open\n", "### Still open\n\n1. Shard the cache?\n- TTL too?\n"
    )
    doc = rfc.parse(write_rfc(repo, text))
    got = rfc.open_questions(doc)
    assert [t for _, t in got] == ["1. Shard the cache?", "- TTL too?"]
    assert doc.lines[got[0][0] - 1] == "1. Shard the cache?"


def test_html_comments_each_reported_even_unclosed(repo, ready_text):
    text = (
        ready_text.replace(
            "## 8. Supply chain\n", "## 8. Supply chain\n<!-- MANDATORY.\nfill -->\n"
        )
        + "\n<!-- left open"
    )
    got = rfc.html_comments(rfc.parse(write_rfc(repo, text)))
    assert len(got) == 2 and got[0][1].startswith("MANDATORY")


def test_symbol_extraction_skips_fences_commands_and_prose(repo):
    toks = [t for _, t in rfc.symbols(rfc.parse(repo / "docs/rfc/0001-cache-eviction.md"))]
    assert toks == ["load_config()", "src/thing.py", "max_entries"]


def test_looks_like_code():
    yes = ["a::b", "src/x.rs", "run()", "snake_case", "JevBackend"]
    no = ["curl -sI", "ab", "Rfc", "README", "plain", "crates/…"]
    assert all(rfc.looks_like_code(t) for t in yes)
    assert not any(rfc.looks_like_code(t) for t in no)


def test_symbol_exists_in_code_and_paths(repo):
    assert rfc.symbol_exists(repo, "load_config()")
    assert rfc.symbol_exists(repo, "src/thing.py")
    assert not rfc.symbol_exists(repo, "no_such_symbol")


def test_symbol_only_in_markdown_is_missing(repo, ready_text):
    write_rfc(repo, ready_text + "\n`only_in_docs_xyz`\n")
    (repo / "docs" / "other.md").write_text("only_in_docs_xyz\n")
    assert not rfc.symbol_exists(repo, "only_in_docs_xyz")


def test_blocks_are_found_by_title_when_numbering_closes_up(repo, ready_text):
    text = (
        ready_text.replace("## 14. Test plan", "## 13. Test plan")
        .replace("## 15. Decisions", "## 14. Decisions")
        .replace("### Still open\n", "### Still open\n\n1. Shard the cache?\n")
    )
    doc = rfc.parse(write_rfc(repo, text))
    assert rfc.missing_headings(doc) == []
    assert [t for _, t in rfc.open_questions(doc)] == ["1. Shard the cache?"]


def test_a_number_with_the_wrong_title_is_missing(repo, ready_text):
    doc = rfc.parse(write_rfc(repo, ready_text.replace("## 8. Supply chain", "## 8. Alternatives")))
    assert rfc.missing_headings(doc) == ["8"]
