# Taskfile layout

## 1. The root file

```yaml
version: "3"
includes:
  mise: .tasks/mise.yaml                               # prefixed: mise:install
  rust: { taskfile: .tasks/rust.yaml, flatten: true }  # flattened: build, fmt
  ui:   { taskfile: .tasks/ui.yaml,   flatten: true }  # names carry ui: themselves
dotenv: [ .env ]
```

batlehub includes fifteen domain files this way (`rust`, `test`, `coverage`, `security`, `ui`,
`docs`, `helm`, `rfc`, `compose`, `browser`, `dex`, `perf`, `release`, and `cde`/`mise` prefixed).
The root file keeps only the includes, global `vars`, and the cross-domain tasks (`init`, `recu`,
`lint`).

What flattening gives:

- Included files inherit the root `vars` and `dotenv`, resolve a relative `dir:` against the root,
  and call each other's tasks by plain name (`- task: ui:install`).
- Two flattened tasks with one name make Task refuse to load (`Found multiple tasks (lint)
  included by "rust"`). So a flattened domain file names its tasks with its own prefix
  (`ui:build`, `ui:lint`, `rust:lint`), except the few it owns outright (`build`, `test`, `fmt`).
- Moving a task between files never renames it, so CI, docs, the devfile and the hooks keep working.

A global var shared by several domain files (batlehub's `BROWSER_EXEC`, used by both
`browser.yaml` and `ui.yaml`) goes in the root `vars:` with a comment saying who uses it.

## 2. Writing a task

```yaml
  release:prepare:
    desc: "Prepare a release (does not push). Pass args after --, e.g. task release:prepare -- --minor --dry-run"
    cmds:
      - bash scripts/prepare-release.sh {{.CLI_ARGS}}
```

- `desc` is mandatory. Put usage and defaults in it: `(usage: task x [BASE=origin/main])`.
- Defaults: `vars: { X: '{{.X | default "y"}}' }`, so `task x X=z` overrides.
- Multi-line shell: `- |` with `set -euo pipefail` on the first line.
- Preconditions, not `test -f … || exit` inside `cmds`:
  ```yaml
  preconditions:
    - sh: "command -v jq >/dev/null"
      msg: "jq is required, install it with 'mise use jq'"
  ```
- Long WHY comments go above `cmds:`, as in `clean:stale` in `templates/.tasks/rust.yaml`.
- Scratch output goes under `.task/` (already ignored) or `/tmp`, never the tree.

## 3. `.tasks` vs `.task`

| Path | What | Git |
| --- | --- | --- |
| `.tasks/` | the included Taskfiles, plus `.tasks/scripts/` for helpers too long for a `cmds:` block | tracked |
| `.task/` | Task's checksum/fingerprint cache (`sources:`/`generates:`), and per-clone scratch a task writes (a fetched template, an export) | ignored |

A Taskfile written into `.task/` is lost the first time someone cleans the cache, and never reaches
review.

## 4. `init`, `recu`, `lint`

```yaml
  init:
    desc: "Init the project: tools, git hooks"
    cmds:
      - task mise:install
      - cog install-hook -a -o     # -a both hooks, -o overwrite stale ones
  recu:
    desc: "Recurring job like regenerating the OpenAPI spec (run by the pre-commit hook)"
    cmds:
      - task: dump-spec            # whatever is generated and tracked
  lint:
    desc: "Lint the whole project (run by the pre-commit hook)"
    cmds:
      - task: rust:lint
      - task: ui:lint
```

`recu` and `lint` must be idempotent and quick: they run on every commit. A `recu` with nothing to
do yet echoes so, rather than not existing, because the hook calls it.
