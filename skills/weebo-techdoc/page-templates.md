# Page templates

Every page is `.md` (never `.mdx`: the markers are HTML comments, which break
MDX). Frontmatter carries `title` and `description`; Fumapress renders both, so
the body starts at `##` and never repeats the title as `#`.

Everything the skill writes sits inside a marked block. Each marker goes on a
line of its own, never indented (4 spaces would make it a code block);
`techdoc.py check` flags a marker sharing a line with anything else:

```md
<!-- techdoc:begin <page>#<section> -->
...generated content...
<!-- techdoc:end -->
```

`<page>` is the path under `docs/techdoc/` without `.md` (`index`,
`components/api`, `components/backend/auth`). Text outside the markers belongs
to the user and is never modified; `scripts/techdoc.py merge` enforces that.
Keep one heading per block, inside it, so a user can write notes between
sections.

References to code are a path plus a backticked symbol, never a line number:
`api/src/routes.rs` `fn list_items`. Line numbers move on every edit and would
make every block look changed on re-run.

## Every page: the notice

The first block after the frontmatter, on every page:

```md
<!-- techdoc:begin <page>#notice -->
> Sections between `techdoc` markers are regenerated; write your own notes outside them.
<!-- techdoc:end -->
```

## index.md

```md
---
title: <Project name>
description: <one sentence: what it is and who uses it>
techdoc_commit: <git rev-parse HEAD at generation time>
---
<!-- techdoc:begin index#notice -->
> Sections between `techdoc` markers are regenerated; write your own notes outside them.
<!-- techdoc:end -->

<!-- techdoc:begin index#purpose -->
## Purpose
<one paragraph: the problem it solves, its main users, where it runs>
<!-- techdoc:end -->

<!-- techdoc:begin index#stack -->
## Stack
<table: layer | technology | where (path)>
<!-- techdoc:end -->

<!-- techdoc:begin index#architecture -->
## Architecture
<structure flowchart, see diagrams.md; one paragraph reading it>
<!-- techdoc:end -->

<!-- techdoc:begin index#components -->
## Components
<table: component | role in one line | link to components/<name>>
<!-- techdoc:end -->
```

## getting-started.md

```md
---
title: Getting started
description: Set up, run, test and change <project>
---
<!-- techdoc:begin getting-started#notice -->
> Sections between `techdoc` markers are regenerated; write your own notes outside them.
<!-- techdoc:end -->

<!-- techdoc:begin getting-started#prerequisites -->
## Prerequisites
<tools from mise.toml / devfile, how to install: `mise install`>
<!-- techdoc:end -->

<!-- techdoc:begin getting-started#tasks -->
## Tasks
<table: task | what it does (from its desc)>, only the ones a developer uses
<!-- techdoc:end -->

<!-- techdoc:begin getting-started#run -->
## Run and test locally
<exact commands, in order>
<!-- techdoc:end -->

<!-- techdoc:begin getting-started#config -->
## Configuration
<table: env var / config key | read by (path + symbol) | default | required?>
<!-- techdoc:end -->

<!-- techdoc:begin getting-started#where-to-change -->
## Where to change things
<"to add an API route, start in `path` `symbol`" style pointers>
<!-- techdoc:end -->
```

## components/\<name\>.md

`<name>` is the component's directory name in kebab-case (`api/` → `api`), so
it stays the same across runs.

```md
---
title: <Component name>
description: <its role in one sentence>
---
<!-- techdoc:begin components/<name>#notice -->
> Sections between `techdoc` markers are regenerated; write your own notes outside them.
<!-- techdoc:end -->

<!-- techdoc:begin components/<name>#role -->
## Role
<what it does, where it lives (path), what depends on it>
<!-- techdoc:end -->

<!-- techdoc:begin components/<name>#interfaces -->
## Interfaces
<what it exposes: HTTP routes (method, path, handler), CLI commands, events or
queues, exported library API. Table per kind.>
<!-- techdoc:end -->

<!-- techdoc:begin components/<name>#resources -->
## Resources
<what it calls: databases, external APIs, other components, filesystem, cache.
Integrations flowchart, see diagrams.md.>
<!-- techdoc:end -->

<!-- techdoc:begin components/<name>#config -->
## Configuration
<env vars and config keys this component reads>
<!-- techdoc:end -->

<!-- techdoc:begin components/<name>#flows -->
## Main flows
<one sequenceDiagram per main flow, each with a sentence above it>
<!-- techdoc:end -->

<!-- techdoc:begin components/<name>#data-model -->
## Data model
<erDiagram or classDiagram; omit the block entirely if the component owns no
persisted or exchanged model>
<!-- techdoc:end -->

<!-- techdoc:begin components/<name>#internals -->
## Internals
<only when the component is complex: its sub-parts as a flowchart, or links to
sub-pages in components/<name>/>
<!-- techdoc:end -->
```

Omit `#data-model` and `#internals` when they would be empty; don't write
placeholder text.

## Orphaned component page

When the code a component page describes no longer exists, never delete the
page. Write a scratch page holding only the frontmatter, the notice block and
the block below, then `techdoc.py merge --partial <page> <scratch>`: the
warning lands right after the notice and every other block stays as it was,
without being reported stale.

```md
<!-- techdoc:begin components/<name>#orphaned -->
> **Warning:** the code this page describes was not found at <commit>. It was
> probably removed or renamed. Delete this page or move your notes.
<!-- techdoc:end -->
```

## maintenance.md

```md
---
title: Maintenance
description: Health checks against the Weebo Dev Env guideline, last run <date>
---
<!-- techdoc:begin maintenance#notice -->
> Sections between `techdoc` markers are regenerated; write your own notes outside them.
<!-- techdoc:end -->

<!-- techdoc:begin maintenance#table -->
## Health checks

| Check | Status | What's there | What's missing | Last run |
| --- | --- | --- | --- | --- |
| ... one row per entry in checks.md ... |

<counts: N ✅ · N ⚠️ · N ❌>
<!-- techdoc:end -->
```

The header row is exact; `techdoc.py check` looks for it.

## meta.json

One per folder that holds pages, setting sidebar order:

```json
{ "pages": ["index", "getting-started", "components", "maintenance"] }
```

`components/meta.json` lists components in the order a reader should meet them
(entry points first). Each entry is a page name without `.md` or a subfolder.
Keep entries the user added; append new ones; drop an entry only when its page
is gone.
