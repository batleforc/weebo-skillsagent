# weebo-techdoc Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `weebo-techdoc` skill that writes and re-runs technical docs (overview, getting started, per-component pages with inline Mermaid, a maintenance health-check table) into a project's `docs/techdoc/`, keeping hand-written text across re-runs.

**Architecture:** The skill is prose (`SKILL.md` + three sibling reference files) driving the agent. The two guarantees the agent must not get wrong are made deterministic in one stdlib-only script, `scripts/techdoc.py`: `merge` (regenerate marked blocks, keep everything else byte-for-byte) and `check` (output structure is valid for Fumapress and for re-runs). An end-to-end eval runs the skill twice on a fixture repo through `claude -p`.

**Tech Stack:** Markdown skill files; Python ≥3.12 stdlib only for the script; pytest + ruff via `uvx`; Taskfile; Claude Code CLI 2.1.x for the eval.

**Spec:** `docs/superpowers/specs/2026-10-16-weebo-techdoc-design.md`

## Global Constraints

- Output lives only in `docs/techdoc/`; nothing outside it is created or modified.
- Pages are `.md`, never `.mdx`. Frontmatter has non-empty `title` and `description`. Body starts at `##`; no `# ` heading.
- `meta.json` in every output folder: `{ "pages": [...] }`.
- Generated blocks: `<!-- techdoc:begin <page>#<section> -->` … `<!-- techdoc:end -->`. Text outside markers is the user's and is never modified.
- Mermaid as fenced ` ```mermaid ` blocks inside pages; standard Mermaid 11; no `%%{init}%%`.
- Code references are `path` + symbol, no line numbers.
- Maintenance table header, exactly: `| Check | Status | What's there | What's missing | Last run |`; status is ✅ / ⚠️ / ❌.
- Publishing `docs/` to the IDP is not checked.
- Runs interactive or unattended: no step may require an answer; judgment calls go in the final plain-text report.
- `techdoc.py` uses the standard library only (it runs in arbitrary target projects with plain `python3`).
- Every Taskfile task has a `desc`; shared tasks go in `.tasks/`.
- **No commits by the agent.** Each task ends by staging the files and writing the message to `.task/commit-msg.txt`; the user runs `git commit -s -S -F .task/commit-msg.txt`. Messages are Conventional Commits and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Prefix shell commands with `rtk` (git, tests) per the repo's CLAUDE.md.

## Review Focus

1. **Hand-written text before the first marker, between two blocks, and after the last** must come out byte-identical after `merge`. Pinned in Task 2 (`test_text_outside_markers_is_byte_identical`).
2. **Marker-looking text inside a code fence** (a page documenting the markers themselves) must not be treated as a marker by `merge` or `check`. Pinned in Task 1 (`test_markers_inside_fence_are_ignored`) and Task 2 (`test_fenced_marker_is_user_text`).
3. **The user deleted an end marker or duplicated an id.** `merge` must refuse (exit 2, nothing written) rather than eat the rest of the page. Pinned in Task 2 (`test_unbalanced_old_refuses`, `test_cli_merge_refuse_leaves_file_untouched`).
4. **The user added their own frontmatter keys** (e.g. `icon`). `title`/`description` come from the new page; other old keys are kept. Pinned in Task 2 (`test_frontmatter_keeps_user_keys`).
5. **CRLF files and a missing trailing newline** survive `merge` unchanged outside markers. Pinned in Task 2 (`test_crlf_and_no_trailing_newline_preserved`).

---

## File map

| File | Responsibility |
| --- | --- |
| `skills/weebo-techdoc/scripts/techdoc.py` | `parse_blocks`, `merge_page`, `check_tree`, CLI `check` / `merge` |
| `skills/weebo-techdoc/tests/test_techdoc.py` | unit tests for the script |
| `skills/weebo-techdoc/page-templates.md` | frontmatter, sections and marker ids per page type |
| `skills/weebo-techdoc/diagrams.md` | perspectives, Mermaid type per purpose, split rule, decomposition |
| `skills/weebo-techdoc/checks.md` | the maintenance checks, one entry each |
| `skills/weebo-techdoc/SKILL.md` | the procedure: discover → write/update → run checks → validate → report |
| `skills/weebo-techdoc/tests/fixtures/sample/` | tiny two-component repo for the eval |
| `skills/weebo-techdoc/tests/eval.sh` | two-run end-to-end eval through `claude -p` |
| `.tasks/techdoc.yaml` | `techdoc:test`, `techdoc:eval` |
| `Taskfile.yml` | include `techdoc`; ruff over the script in `lint` |
| `profiles.toml`, `readme.md` | new `docs` profile; item + profile rows |

---

### Task 1: `techdoc.py check`

**Files:**
- Create: `skills/weebo-techdoc/scripts/techdoc.py`
- Create: `skills/weebo-techdoc/tests/test_techdoc.py`
- Create: `.tasks/techdoc.yaml`
- Modify: `Taskfile.yml` (add `techdoc: ./.tasks/techdoc.yaml` to `includes`; add `uvx ruff check skills/weebo-techdoc` and `uvx ruff format --check skills/weebo-techdoc` to `lint`)

**Interfaces:**
- Produces:
  - `class MarkerError(ValueError)`
  - `BEGIN_RE = re.compile(r"^<!-- techdoc:begin (\S+) -->$")`, `END = "<!-- techdoc:end -->"` (matched against a stripped line)
  - `parse_blocks(text: str) -> dict[str, str]`: block id → inner text (between the marker lines, exclusive). Ignores lines inside ``` / ~~~ fences. Raises `MarkerError` on: begin inside a block, end without begin, begin never closed, duplicate id.
  - `check_tree(root: Path) -> list[str]`: problems as `"<relative path>: <message>"`, empty = valid.
  - CLI: `python3 techdoc.py check DIR` → prints problems, exit 0 if none, 1 otherwise, 2 if DIR missing.

`check_tree` rules (one test each):
1. Any `*.mdx` under root is a problem.
2. Each `*.md` starts with `---` frontmatter containing non-empty `title:` and `description:` (line-based `key: value` parse; no YAML dependency).
3. No line starting with `# ` outside fences.
4. `parse_blocks` succeeds (its `MarkerError` message becomes the problem).
5. Each ` ```mermaid ` fence's first non-blank line starts with one of `flowchart`, `graph`, `sequenceDiagram`, `classDiagram`, `erDiagram`, `stateDiagram`, `stateDiagram-v2`, `C4Context`, `C4Container`, `C4Component`, `journey`, `gitGraph`, `mindmap`, `timeline`; the fence contains no `%%{init`.
6. Every folder holding a `.md` has a `meta.json` with a list `pages`; each entry names an existing `<entry>.md` or subfolder.
7. If `maintenance.md` exists it contains the exact table header from Global Constraints.

- [ ] **Step 1: Write failing tests** in `test_techdoc.py` (import the script via `importlib.util.spec_from_file_location` from `../scripts/techdoc.py`; build trees in `tmp_path`):
  - `test_valid_tree_has_no_problems` — a root with `meta.json` `{"pages":["index","components"]}`, `index.md` (frontmatter, `## Overview`, a marked block with a `flowchart LR` mermaid fence), `components/meta.json`, `components/api.md` → `check_tree(root) == []`.
  - `test_mdx_rejected`, `test_missing_description`, `test_h1_rejected` (but `# x` inside a fence is fine), `test_unknown_mermaid_type` (`pie2 title x`), `test_mermaid_init_rejected`, `test_meta_json_missing`, `test_meta_json_dangling_entry`, `test_maintenance_header_required` — each asserts exactly one problem whose text contains the file name.
  - `test_parse_blocks_ok` — two blocks → `{"index#a": "x\n", "index#b": "y\n"}`.
  - `test_parse_blocks_errors` — parametrized over unclosed begin, stray end, nested begin, duplicate id → `pytest.raises(MarkerError)`.
  - `test_markers_inside_fence_are_ignored` — a ```` ```md ```` fence containing a begin marker with no end → `parse_blocks` returns `{}`.
  - `test_cli_check_exit_codes` — `subprocess.run([sys.executable, SCRIPT, "check", root])` → 0 valid, 1 invalid, 2 missing dir.
- [ ] **Step 2:** `rtk test uvx pytest skills/weebo-techdoc/tests -q` → FAIL (script missing).
- [ ] **Step 3:** Implement the interfaces above in `techdoc.py` with `argparse` subcommands. Add `.tasks/techdoc.yaml` task `test` (desc: "Run the weebo-techdoc script tests (no network, no LLM)", cmd: `uvx pytest skills/weebo-techdoc/tests -q`) and the `Taskfile.yml` changes.
- [ ] **Step 4:** `task techdoc:test` → PASS; `task lint` → PASS.
- [ ] **Step 5:** Stage; write `.task/commit-msg.txt`: `feat(techdoc): add output checker for weebo-techdoc`.

### Task 2: `techdoc.py merge`

**Files:**
- Modify: `skills/weebo-techdoc/scripts/techdoc.py`
- Modify: `skills/weebo-techdoc/tests/test_techdoc.py`

**Interfaces:**
- Consumes: `parse_blocks`, `MarkerError` (Task 1).
- Produces:
  - `merge_page(old: str, new: str) -> tuple[str, list[str]]` → (merged text, stale ids).
  - CLI: `python3 techdoc.py merge OLD NEW [-o OUT]` → writes merged to OUT (default: overwrite OLD) only on success; stale ids printed one per line to stderr as `stale: <id>`; exit 0. `MarkerError` in either input → message on stderr, exit 2, nothing written.

Merge rules:
- Frontmatter: start from old's key/value lines; `title` and `description` take new's values; keys only in new are appended. If old has no frontmatter, use new's.
- Body: walk old line by line. Outside markers, copy the line verbatim (original line endings). For each old block whose id is in new, emit old's begin line, new's inner text, old's end line. Old blocks absent from new are kept verbatim and returned as stale.
- Blocks only in new: insert each right after the end line of the nearest preceding block (by new's order) that exists in the output; if none precedes, after the frontmatter. Separate with one blank line.

- [ ] **Step 1: Write failing tests:**
  - `test_text_outside_markers_is_byte_identical` — old = fm + `"intro\n"` + block A(`"old a\n"`) + `"between\n"` + block B + `"outro"`; new has A=`"new a\n"`, B=`"new b\n"` → result equals old with only the two inner texts swapped; `stale == []`.
  - `test_new_block_inserted_after_predecessor` — new has A, C, B; old has A, B → C appears right after A's end line.
  - `test_missing_block_kept_and_reported` — old has A, B; new has A → B kept verbatim; `stale == ["p#b"]`.
  - `test_fenced_marker_is_user_text` — a fenced example begin/end pair in old outside any block is copied unchanged.
  - `test_unbalanced_old_refuses` — old missing an end → `pytest.raises(MarkerError)`.
  - `test_frontmatter_keeps_user_keys` — old fm has `title: Old`, `icon: Server`; new has `title: New`, `description: D` → merged fm has `title: New`, `description: D`, `icon: Server`.
  - `test_crlf_and_no_trailing_newline_preserved` — old uses `\r\n` and has no final newline → those bytes outside blocks are unchanged.
  - `test_cli_merge_refuse_leaves_file_untouched` — invalid OLD → exit 2, file bytes unchanged.
- [ ] **Step 2:** `task techdoc:test` → new tests FAIL.
- [ ] **Step 3:** Implement `merge_page` and the `merge` subcommand. Read and write with `newline=""` so line endings survive.
- [ ] **Step 4:** `task techdoc:test` → PASS; `task lint` → PASS.
- [ ] **Step 5:** Stage; commit message `feat(techdoc): merge regenerated blocks while keeping hand-written text`.

### Task 3: reference files

**Files:**
- Create: `skills/weebo-techdoc/page-templates.md`
- Create: `skills/weebo-techdoc/diagrams.md`
- Create: `skills/weebo-techdoc/checks.md`

**Interfaces:**
- Produces: section ids used in markers (Task 4's SKILL.md and Task 5's eval grep these):
  - `index#purpose`, `index#stack`, `index#architecture`, `index#components`
  - `getting-started#prerequisites`, `#tasks`, `#run`, `#config`, `#where-to-change`
  - `components/<name>#role`, `#interfaces`, `#resources`, `#config`, `#flows`, `#data-model`, `#internals`
  - `maintenance#table`
- Produces: `checks.md` entries with exactly these fields: `### <Check name>`, `Inspect:`, `Run:` (command or `—`), `Grade:` (✅ / ⚠️ / ❌ conditions).

Content requirements (no tests; checked by Task 4's validation and Task 5's eval):

- **page-templates.md**: for each page type, a skeleton showing frontmatter, the `##` headings, and marker placement. `index.md` frontmatter also carries `techdoc_commit: <sha>`. Each page's first line after frontmatter is a marked `<page>#notice` block: "Sections between `techdoc` markers are regenerated; write your own notes outside them." Orphaned component pages get a marked `<page>#orphaned` warning block at the top. References written as `path` + backticked symbol.
- **diagrams.md**: three perspectives (structure → `flowchart` in `index#architecture`; data flow → `sequenceDiagram` per main flow in `#flows`; external integrations → `flowchart` in `#resources`); `erDiagram`/`classDiagram` for `#data-model` only when the code defines persisted or exchanged models; split rule: more than ~15 nodes → move a subgraph to `#internals` or a subfolder page (oh-my-mermaid's recursive decomposition); stop decomposing when a node needs no diagram; node labels use real names from the code; Mermaid 11 syntax, no `%%{init}%%`, quote labels containing punctuation.
- **checks.md**: the nine checks from the spec's table (audit, gen:api-docs, CI/CD, automated review, lint, tests, commit hooks, Containerfile, dependencies) with the spec's inspect/run text and these grading calls: missing task/tool → ❌ "not found"; command timeout (600 s, via `timeout 600`) → ⚠️ "timed out"; automated review ✅ only when triggered by CI or a git hook, ⚠️ when only a manual task; command changed tracked files (`git status --porcelain` before/after) → note it in the row, never revert or commit; CI/CD row never checks IDP publishing. A closing section "Adding a check" says one new `###` entry is all it takes.

- [ ] **Step 1:** Write the three files.
- [ ] **Step 2:** Self-check: every section id above appears in `page-templates.md`; `checks.md` has nine `###` entries each with the four fields (`grep -c '^### ' skills/weebo-techdoc/checks.md` → 9).
- [ ] **Step 3:** Stage; commit message `docs(techdoc): page templates, diagram rules and maintenance checks`.

### Task 4: SKILL.md, profile, readme

**Files:**
- Create: `skills/weebo-techdoc/SKILL.md`
- Modify: `profiles.toml` (new `[profiles.docs]`, `include = ["base"]`, `items = ["weebo-techdoc"]`, one comment line)
- Modify: `readme.md` (profiles table row `docs | base | weebo-techdoc`; items table row: symlink, ours, needs `python3`, `git`)

**Interfaces:**
- Consumes: section ids and `checks.md` fields (Task 3); `techdoc.py check|merge` (Tasks 1–2).

Frontmatter: `name: weebo-techdoc`, a `description` that triggers on "write/update the technical docs", "document the architecture", "techdoc", "maintenance checklist"; `user-invocable: true`; `argument-hint: "[path to project, default .]"`.

Body, in order (point to sibling files, don't repeat them):
1. **What it writes** — the layout under `docs/techdoc/`, nothing outside it.
2. **Discover** — read the repo's `CLAUDE.md`/`AGENTS.md`, Taskfile(s), manifests, entry points; decide components (deployable, crate/package, or module with its own interface). If `docs/techdoc/index.md` has `techdoc_commit`, use `git diff --name-only <sha>..HEAD` to limit which pages get regenerated; otherwise regenerate all.
3. **Write** — generate each page per `page-templates.md` and `diagrams.md` into a scratch file; if the page exists, `python3 <skill>/scripts/techdoc.py merge <existing> <scratch>`; on exit 2, leave the page untouched and report it. New page → write directly. Component gone → add the orphaned block via merge, never delete. Update `meta.json` files and `techdoc_commit`.
4. **Maintenance** — run every entry in `checks.md`, fill the table into `maintenance#table`.
5. **Validate** — `python3 <skill>/scripts/techdoc.py check docs/techdoc`; fix and re-run until exit 0.
6. **Report** — plain text: pages created / updated / refused by merge / orphaned; maintenance summary (counts per status, every ❌); tracked files changed by commands; judgment calls made. No questions in unattended runs; in a session, ask only before something expensive to undo.
7. **Never** — commit, touch files outside `docs/techdoc/` (besides what check commands themselves change), invent symbols, put line numbers in references.

- [ ] **Step 1:** Write `SKILL.md`, the profile and readme rows.
- [ ] **Step 2:** `task skills:doctor` → `weebo-techdoc` healthy; `task skills:list PROFILES=1` shows `docs`.
- [ ] **Step 3:** Stage; commit message `feat(techdoc): add weebo-techdoc skill and docs profile`.

### Task 5: fixture and end-to-end eval

**Files:**
- Create: `skills/weebo-techdoc/tests/fixtures/sample/` — `Taskfile.yml` (tasks `audit`: `echo "cargo audit: 0 vulnerabilities"`, `lint`: `echo ok`, `test`: `echo ok`; **no** `gen:api-docs`), `api/src/main.rs` (two route handlers `list_items`, `create_item` registered on `/items`, reading `DATABASE_URL`), `web/src/client.ts` (fetches `/items`), `readme.md`. No need to compile.
- Create: `skills/weebo-techdoc/tests/eval.sh`
- Modify: `.tasks/techdoc.yaml` (task `eval`, desc: "Run weebo-techdoc twice on the fixture through claude -p (costs API credits; not for CI)")

**Interfaces:**
- Consumes: the whole skill; `techdoc.py check`.

`eval.sh` (bash, `set -euo pipefail`), in a `mktemp -d` work dir:
1. Copy the fixture, `git init`, commit it; symlink the skill into `.claude/skills/weebo-techdoc`.
2. Run 1: `claude -p "Use the weebo-techdoc skill on this repository." --permission-mode bypassPermissions --max-budget-usd 3`.
3. Assert: `techdoc.py check docs/techdoc` exits 0; `grep -rq list_items docs/techdoc/components`; `grep -q 'DATABASE_URL' -r docs/techdoc`; the `gen:api-docs` row in `maintenance.md` contains ❌; `git status --porcelain` shows changes only under `docs/techdoc/`.
4. Append `MY HAND NOTE` after the last marker in `docs/techdoc/index.md`; add a third handler `delete_item` to `api/src/main.rs`; commit.
5. Run 2: same command.
6. Assert: `grep -q 'MY HAND NOTE' docs/techdoc/index.md`; `grep -rq delete_item docs/techdoc/components`; `check` exits 0.
7. Print `EVAL PASS` or the failed assertion; exit non-zero on failure.

- [ ] **Step 1:** Write the fixture and `eval.sh`; add the `eval` task.
- [ ] **Step 2:** `task techdoc:eval` → `EVAL PASS`. If it fails, fix the skill text (Tasks 3–4), not the assertions; record what changed.
- [ ] **Step 3:** `task techdoc:test` and `task lint` → PASS.
- [ ] **Step 4:** Stage; commit message `test(techdoc): fixture repo and two-run end-to-end eval`.
