---
title: "weebo-techdoc — design"
---

Status: draft, awaiting approval · 2026-10-16

## Intent

A skill, `weebo-techdoc`, that writes and keeps up to date the **technical
documentation** of a project, as Markdown in that project's `docs/` folder.
CI pushes `docs/` to the user's new IDP, whose Fumapress engine renders every
project's docs (Mermaid included). The project repo holds only Markdown, never
a site.

Readers: a new developer joining, the user six months from now, an architect
who must understand the system. **No end users.** They come for three things:

1. how the project works as a whole;
2. how each part works: its interfaces and the resources it calls;
3. how to start developing on it.

Plus a fourth page for whoever maintains the project: a filled-in health-check
table against the Weebo Dev Env guideline (audit, API doc generation, CI/CD,
automated review, …).

## Goals

- Docs derived from reading the code: real file paths, routes, commands, env
  vars. Nothing invented; anything unconfirmed is marked as such on the page.
- Mermaid diagrams written inline as fenced ` ```mermaid ` blocks inside the
  pages, using oh-my-mermaid's approach: several perspectives (structure, data
  flow, external integrations), and recursive decomposition where a part is
  complex enough to deserve its own diagram.
- Re-runnable: a second run updates only what changed and never touches text
  the user wrote by hand.
- A maintenance page that the skill fills in by inspecting the repo **and
  running the commands**, presented as a table.
- The list of maintenance checks is extensible without rewriting the skill.

## Non-goals

- End-user docs, tutorials or marketing pages.
- Building, configuring or deploying a Fumapress site (no `press.config.tsx`,
  no Mermaid plugin setup, no `package.json`). The IDP renders.
- oh-my-mermaid's `.omm/` layout, separate `.mmd` files, its CLI or cloud.
- Fixing the gaps the maintenance table finds. It reports; it does not repair.

## Anti-goals (failure even if it "works")

- Docs that describe what the code *should* do, or invent symbols/routes that
  do not exist.
- A re-run that overwrites or deletes the user's hand-written prose.
- Diagrams kept outside the pages, or so large they are unreadable (split
  instead).
- A maintenance table filled from guesses instead of from what was found and
  run.

## Output layout (in the target project)

All techdoc output lives in its own subfolder, `docs/techdoc/`, so it never
collides with pages, a Fumadocs site or other files already in `docs/`.

```
docs/techdoc/
  meta.json               # sidebar order for the techdoc pages
  index.md                # overview: what it is, stack, architecture diagram
  getting-started.md      # mise tools, key task commands, run/test, where to change things
  components/
    meta.json
    <component>.md        # one page per part
    <group>/              # subfolders allowed when the code is organised that way
      meta.json
      <component>.md
  maintenance.md          # health-check table
```

- Every page: frontmatter with `title` and `description` (Fumapress shows
  both); body starts at `##`, never repeats the title as `#`.
- `meta.json` (`{ "pages": [...] }`) in each folder sets sidebar order.
- Pages are `.md`, not `.mdx` (the update markers below are HTML comments,
  which break MDX).
- Nothing outside `docs/techdoc/` is created or modified, including any
  `docs/meta.json` (how `techdoc/` appears in the IDP sidebar is the IDP's
  concern).

### Page contents

- **index.md**: purpose in a paragraph, stack, a `flowchart` of the main parts
  and the outside services they talk to, links to each component page.
- **getting-started.md**: prerequisites via mise, the Taskfile commands that
  matter (taken from `desc` fields), how to run and test locally, required env
  vars/config, and "to change X, start in file Y" pointers.
- **components/\<name\>.md**: role; interfaces exposed (HTTP routes, CLI
  commands, events/queues, exported library API); resources called
  (databases, external APIs, other components, filesystem); config/env it
  reads; diagrams: a `sequenceDiagram` per main flow, an `erDiagram` or
  `classDiagram` where a data model exists, a sub-`flowchart` when the part
  is complex. Each claim carries a `path` + symbol reference (e.g.
  `src/api/routes.rs` `fn list_items`). *(My call: no line numbers — they
  move on every edit and would make every block look changed on re-run.)*
- **maintenance.md**: see below.

What counts as a "component" and how deep to decompose is left to the builder
(guidance: a deployable, a crate/package, or a module with its own interface;
stop decomposing when a node needs no diagram to understand).

## Updating on re-run

- Every block the skill writes is wrapped in markers:
  `<!-- techdoc:begin <page>#<section> -->` … `<!-- techdoc:end -->`.
- On re-run the skill regenerates only marked blocks whose source changed;
  everything outside markers is the user's and is never modified. A marked
  block the user edited by hand is overwritten. *(My call: the page header
  says so in one line, so the rule is visible to people editing.)*
- New code parts → new pages. A part that disappeared → its page is **not**
  deleted; a warning callout is inserted at the top and the skill lists it in
  its final report.
- `maintenance.md` is fully re-run every time; its table is one marked block.

How "source changed" is detected (git diff since a recorded commit, content
hashing, or plain regeneration and compare) is left to the builder. *(My call:
recording the last-run commit in the frontmatter of `index.md` is acceptable.)*

## maintenance.md

A table, one row per check:

| Check | Status | What's there | What's missing | Last run |
| --- | --- | --- | --- | --- |

Status is ✅ / ⚠️ / ❌. "Last run" holds the date and outcome of the command
the skill actually ran (pass/fail/not found, with a one-line excerpt on
failure).

Initial checks:

| Check | Inspect | Run |
| --- | --- | --- |
| `task audit` | which scanners, do they cover every stack in the repo (cargo audit/deny, pnpm audit, trivy/grype) | `task audit` |
| `task gen:api-docs` | does a schema exist, how is it produced (utoipa + Scalar for Rust, HeyApi client for the front end), is it current | `task gen:api-docs`, then `git status` to see if the committed schema was stale |
| CI/CD | which pipelines exist and what they run. Publishing `docs/` to the IDP is **not** checked here; another skill owns that. | — |
| Automated review | is `judge doc-review` (or equivalent) wired on changes. *(My call: ✅ when triggered by CI or a git hook; ⚠️ when only a manual task exists.)* | — |
| Lint | task exists | `task lint` |
| Tests | task exists | the project's test task |
| Commit hooks | cog `commit-msg`, gitleaks, pre-commit tasks present | — |
| Containerfile | multi-stage build | `trivy` image/config scan if available |
| Dependencies | outdated packages vs "latest unless CVE" | `cargo outdated` / `pnpm outdated` as fits the stack |

The checks live in a file inside the skill (`checks.md`), one entry per
check with: name, what to inspect, what to run, and how to grade
✅/⚠️/❌. Adding a check means adding an entry there, nothing else.

Running commands: a missing tool or task is ❌ "not found", not an error. If a
command modifies tracked files (e.g. `gen:api-docs` regenerating a stale
schema), the skill reports it in the row and in its final message and leaves
the working tree change for the user to review — it does not revert or commit.
*(My call: commands get a time limit; a timeout is ⚠️ with "timed out".)*

## Skill packaging (in this repo)

```
skills/weebo-techdoc/
  SKILL.md          # the procedure: discover → write/update pages → run checks → report
  page-templates.md # frontmatter, section layout and markers per page type
  diagrams.md       # perspectives, which Mermaid type for what, size limits, decomposition rule
  checks.md         # the maintenance checks
```

Follows the repo convention seen in `batlehub-rfc`: SKILL.md points to the
sibling files rather than repeating them. *(My call: add it to
`profiles.toml` under a new `docs` profile that includes `base`; doc-review is
a natural companion but stays in `review`.)*

The skill ends with a short report: pages created/updated, pages flagged as
orphaned, maintenance table summary, any tracked files the commands changed.

## Who runs it

Two modes, same skill:

- **Interactive**: the user invokes it in a session.
- **Unattended**: run from CI (or a `judge`-style gate) with nobody watching.

The skill therefore never depends on asking: every decision (component split,
which commands to run, timeouts) has a default it takes on its own, and every
judgment call goes into the final report instead. *(My call: in an
interactive session it may still ask before a choice that would be expensive
to undo, but the unattended path must reach the same output without asking.)*
The final report must be usable as a CI log: plain text, no prompts.

## Constraints

- Follows the target repo's own `CLAUDE.md`/`AGENTS.md` (Taskfile, mise,
  rtk-prefixed commands, no commits — the user commits).
- No root, no containers: checks needing an unavailable tool degrade to
  ❌/⚠️ with the reason.
- Mermaid must stay valid for the IDP's renderer (standard Mermaid 11
  syntax; no `%%{init}%%` theming, since the IDP owns the look).

## Left to the builder

- Component detection and decomposition depth heuristics.
- Change detection mechanism for re-runs.
- Exact wording and section order inside page templates.
- Diagram size threshold for splitting.
- How the skill is tested/evaluated (a fixture repo is suggested).
