# Maintenance checks

Each `###` entry is one row of the table in `maintenance.md`. Fill the row
from what you **found and ran**, never from what should be there.

How to run every check:

- Run commands from the project root, each under `timeout 600`.
- Before and after each run, record `git status --porcelain`. If a command
  changed tracked files, say so in the row ("changed `openapi.json`") and list
  the files in the final report. Never revert, stage or commit them.
- A task or tool that doesn't exist is ❌ "not found", not an error; move on.
- A timeout is ⚠️ "timed out after 600 s".
- "Last run" holds the date and the outcome (pass / fail / not found /
  timed out), plus the first relevant line of output on failure.
- Only inspect what the project actually uses: a stack with no Rust gets no
  cargo verdict.

### Audit

Inspect: does `task audit` exist? Which scanners does it run, and do they
cover every stack in the repo (`cargo audit` / `cargo deny` for Rust,
`pnpm audit` for JS/TS, `trivy` / `grype` for a Containerfile or image)?
Run: `task audit`
Grade: ✅ task exists, covers every stack, passes · ⚠️ passes but a stack is
not covered, or it reports vulnerabilities · ❌ no task, or it fails to run.

### API docs generation

Inspect: does `task gen:api-docs` exist? Does the backend generate its schema
(utoipa with the Scalar UI for Rust), and does the front end generate its
client from it (HeyApi with the axios plugin)? Where is the schema written?
Run: `task gen:api-docs`, then `git status --porcelain`; changed schema files
mean the committed schema was stale.
Grade: ✅ task exists and the committed schema was current · ⚠️ task exists
but the schema was stale, or only one side (spec or client) is generated ·
❌ the project exposes an API and has no generation. Write "no API" and ✅ if
the project exposes none.

### CI/CD

Inspect: which pipelines exist (`.github/workflows/`, `.forgejo/workflows/`,
`.gitlab-ci.yml`), when they trigger, and which tasks they run (lint, tests,
audit, build, release). Do **not** check whether `docs/` is published to the
IDP; another skill owns that.
Run: —
Grade: ✅ pipelines run lint, tests and audit on changes · ⚠️ pipelines exist
but skip one of those · ❌ no pipeline.

### Automated review

Inspect: is `judge doc-review` (or another automated review) wired on changes:
in a CI job, a git hook, or only as a Taskfile task?
Run: —
Grade: ✅ triggered by CI or a git hook · ⚠️ only a manual task exists ·
❌ none.

### Lint

Inspect: does `task lint` exist, and what does it run?
Run: `task lint`
Grade: ✅ exists and passes · ⚠️ exists and fails · ❌ not found.

### Tests

Inspect: which test task exists (`task test`, or the stack's tests under
`.tasks/`)?
Run: that task.
Grade: ✅ exists and passes · ⚠️ exists and fails, or finds no tests ·
❌ not found.

### Commit hooks

Inspect: is `cog.toml` present with a `commit-msg` hook running `cog verify`?
Does the pre-commit hook run `task recu`, `task lint`, gitleaks and the
whitespace scripts? (`.hooks/`, `cog.toml` `[git_hooks]`.)
Run: —
Grade: ✅ all present · ⚠️ some present · ❌ none.

### Containerfile

Inspect: is there a `Containerfile` / `Dockerfile`? Is it a multi-stage build
with a slim final stage?
Run: `trivy config <file>` if `trivy` is on PATH; otherwise note "trivy not
available".
Grade: ✅ multi-stage and the scan is clean · ⚠️ single-stage, or scan
findings, or scan not possible · ❌ not used for this check: write
"no Containerfile" with ✅ if the project ships no image.

### Dependencies

Inspect: are dependencies on their latest versions, per the guideline "latest
unless it has a CVE"?
Run: `cargo outdated --root-deps-only` (if installed) for Rust, `pnpm outdated`
for JS/TS. Skip any tool that isn't installed and say so.
Grade: ✅ nothing outdated · ⚠️ some outdated, or the tool isn't installed ·
❌ a major version behind on a runtime-critical dependency.

## Adding a check

Add one `###` entry above with the same four parts: name, `Inspect:`, `Run:`
(a command or `—`), `Grade:`. The skill reads this file on every run, so
nothing else changes.
