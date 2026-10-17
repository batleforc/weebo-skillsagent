# Git hooks and .gitignore

## 1. Install

The hooks are scripts in `cog.toml` under `[git_hooks.<name>]`; `cog install-hook -a -o` (part of
`task init`) writes them to `.git/hooks/`. Editing `cog.toml` changes nothing until it is re-run.
`.git/hooks/` is not versioned, so every clone runs `task init` once.

## 2. commit-msg

```sh
cog verify --file "$1"
```

Rejects anything that is not Conventional Commits: `type(scope)!: subject`. Types: `feat`, `fix`,
`chore`, `docs`, `refactor`, `perf`, `test`, `ci`, `build`, `style`, `revert`, plus any declared under
`[commit_types]` in `cog.toml`. The type drives the next version bump (`weebo-release`), so `feat`
vs `fix` matters. Check a message before handing it over: `cog verify --file msg.txt`.

## 3. pre-commit, step by step

| Step | Does | When it fails |
| --- | --- | --- |
| `task recu` | regenerates tracked artefacts | the generator broke; fix it |
| `git add -u` | stages what `recu` rewrote (tracked files only) | |
| `task lint` | every linter and format check | fix the finding, or run the formatter (`task fmt`, `task ui:fix`) |
| `git add -u` | stages what a fixing linter changed | |
| `.hooks/end-of-line.sh` | appends a final newline to staged text files, re-stages them | |
| `.hooks/whitespace-fixer.sh` | strips trailing whitespace from the staged lines, re-stages | aborts with `NO CHANGES ADDED` when nothing is staged |
| `gitleaks git --pre-commit --redact --staged --verbose` | secret scan of the staged diff | a real secret: unstage it and rotate it. A false positive: `.gitleaksignore` (fingerprint) or an allowlist in `gitleaks.toml` (`weebo-security`) |

`set -e` stops at the first failure. Hooks run from the repo root, so the `.hooks/` paths are
relative to it.

`git add -u` re-stages **whole tracked files**, partial staging (`git add -p`) included: whatever
else is modified in those files goes into the commit too. Stash unrelated work first.

## 4. `.hooks/ignore`

Glob patterns, one per line, that the two fixers skip. List content the repo vendors rather than
authors (`skills/*`, `LICENSES/*`): reformatting it diverges it from its upstream, so the next sync
sees a local patch and refuses to update it.

## 5. .gitignore basics

| Entry | Why |
| --- | --- |
| `.task/` | Task's cache and per-clone scratch. Not `.tasks/`. |
| `target/`, `node_modules/`, `dist/` | build output; delete when they grow (CDE disks are small) |
| `coverage/`, `lcov.info` | coverage output (`weebo-testing`) |
| `.env`, `.env.local` | secrets, loaded by Task's `dotenv` |
| `.claude/settings.local.json` | per-user Claude Code settings |
| `__pycache__/`, `*.pyc`, `.venv/`, `*.log` | |

Generated files that should be reviewed as a diff (an OpenAPI spec, `mise.lock`, `Cargo.lock`,
`pnpm-lock.yaml`) are tracked; the code generated from them (a TS client) is ignored
(`weebo-api-contract`). Give every non-obvious entry a comment saying what writes it.
