---
name: weebo-techdoc
description: >
  Write or update a project's technical documentation as Markdown in
  docs/techdoc/: an overview with an architecture diagram, a getting-started
  guide, one page per component (interfaces, resources it calls, config,
  Mermaid flows and data model) and a maintenance health-check table filled by
  running the project's checks (audit, API doc generation, CI/CD, automated
  review, lint, tests, hooks, Containerfile, dependencies). Re-runs update only
  what changed and keep hand-written text. Use when asked to "write the
  techdoc", "document the architecture", "update the technical docs", "run
  the maintenance checklist", or /weebo-techdoc. Not for end-user docs.
user-invocable: true
argument-hint: "[project path, default .]"
---

Technical docs for the people who build and maintain a project: a new
developer, the owner six months later, an architect. Not for end users. The
docs are plain Markdown in `docs/techdoc/`; CI pushes `docs/` to the IDP, whose
Fumapress engine renders them, Mermaid included. You write Markdown only:
never a site, a `press.config.tsx` or a `package.json`.

This file is the procedure. The rest lives beside it; read each in full when
the step names it:

- `page-templates.md`: every page's frontmatter, sections and marker ids.
- `diagrams.md`: which Mermaid diagram goes where, and when to split.
- `checks.md`: the maintenance checks and how to grade them.
- `scripts/techdoc.py`: `merge` (re-runs) and `check` (validation). Standard
  library only; run it with `python3`.

`<skill>` below means this skill's directory.

## You may be running unattended

This skill runs in a session or from CI with nobody watching. Never wait for an
answer: every choice below has a default, so take it and record it in the
report. In a session you may ask before something expensive to undo; the
unattended run must reach the same result without asking.

## 1. Discover

1. Read the project's `CLAUDE.md` / `AGENTS.md` and follow them (`rtk`
   prefixes, mise, no commits).
2. Read the Taskfile(s) (`Taskfile.yml|yaml`, `.tasks/*.yaml`), manifests
   (`Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`), `mise.toml`,
   `devfile.yaml`, Containerfiles, CI files and entry points.
3. Decide the components. A component is a deployable, a crate/package, or a
   module with its own interface. Group them into subfolders under
   `components/` when the code is organised that way (`components/backend/…`).
   Name each page after the component's directory, in kebab-case (`api/` →
   `components/api.md`, `crates/auth-core/` → `components/auth-core.md`).
   The same component must get the same page name on every run, or a re-run
   would create a duplicate page and orphan the old one.
4. Decide what to regenerate:
   - `docs/techdoc/` missing → everything.
   - `docs/techdoc/index.md` has `techdoc_commit: <sha>` and that commit
     exists → `git diff --name-only <sha>..HEAD`. Regenerate the pages whose
     component's files changed, plus `index.md` and `getting-started.md` if
     manifests, Taskfiles, config or the component list changed.
   - Otherwise (no sha, or the sha is gone) → everything. Merging keeps the
     hand-written text either way.

Every claim must come from code you read. Refer to code as a path plus a
backticked symbol (`api/src/routes.rs` `fn list_items`), never line numbers.
If you can't confirm something, say so on the page ("not found in the code")
rather than guessing. Never invent a route, symbol, env var or table.

## 2. Write the pages

For each page to (re)generate, follow `page-templates.md` and `diagrams.md`:

1. Write the full generated page to a scratch file outside the repo.
2. If the page doesn't exist yet, move the scratch file into place.
3. If it exists: `python3 <skill>/scripts/techdoc.py merge <page> <scratch>`.
   - exit 0: merged in place. `stale: <id>` lines on stderr name blocks the
     new version no longer has; they were kept, so list them in the report.
   - exit 2: the page's markers are broken (a user deleted an end marker,
     duplicated an id, or left a code fence unclosed). Leave the page
     untouched and report it.
4. A component that no longer exists: keep its page and add the `#orphaned`
   block with `merge --partial` as `page-templates.md` describes. Never
   delete a page.

Then update each folder's `meta.json` (keep the user's entries, including
Fumadocs separators like `---Guides---` and links; append new pages; drop
entries whose page is gone). Always regenerate `index.md`'s frontmatter with
`techdoc_commit: <git rev-parse HEAD>`: `merge` takes `title`, `description`
and `techdoc_commit` from the new page and keeps every other frontmatter key
the user added.

## 3. Run the maintenance checks

Read `checks.md` and run every entry in it, in order, following its rules for
timeouts and changed files. Fill the table into `maintenance.md` (through
`merge` when the page exists). `maintenance.md` is regenerated on every run.

## 4. Validate

```bash
python3 <skill>/scripts/techdoc.py check docs/techdoc
```

Exit 1 lists problems as `<file>: <message>`. Fix them and run it again until
it exits 0. Don't finish with a failing check.

## 5. Report

End with a plain-text report (it may be a CI log, so no questions and no
interactive formatting):

```
weebo-techdoc report
pages created:   <list>
pages updated:   <list>
pages refused:   <list, with the merge error>   (markers broken; fix by hand)
pages orphaned:  <list>
stale blocks:    <page#id list>
maintenance:     N ✅ · N ⚠️ · N ❌
  ❌ <check>: <what's missing>
files changed by check commands: <list, or none>
judgment calls:  <components chosen, anything unconfirmed, defaults taken>
```

## Never

- Create or modify anything outside `docs/techdoc/`. The only exception is
  files a check command changes by itself, which you report and leave alone.
- Commit, stage or revert anything. The user commits.
- Edit text outside `techdoc` markers, or delete a page.
- Write `.mdx`, `# ` titles in the body, `%%{init}%%` in Mermaid, or line
  numbers in code references.
- Check whether `docs/` is published to the IDP. Another skill owns that.
