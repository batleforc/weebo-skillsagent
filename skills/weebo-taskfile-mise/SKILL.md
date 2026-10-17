---
name: weebo-taskfile-mise
description: The Weebo project skeleton, modelled on batlehub and weebo-skillsagent — a root Taskfile.yml that includes one `.tasks/<domain>.yaml` per domain with `flatten: true` and a `desc` on every task, mise as the only package manager (pins, backends, mise.lock), cocogitto git hooks (commit-msg `cog verify`; pre-commit `task recu`, `task lint`, whitespace/EOL fixers, gitleaks) and the `.gitignore` basics. Use when bootstrapping a Weebo repo, adding a task or a tool, wiring or debugging the git hooks, or when a command is missing and you are tempted to install it by hand.
---

# Weebo project skeleton

Two reference repos: [batlehub](https://github.com/batleforc/batlehub) at commit `20879e5` (the
Taskfile layout, mise.toml) and weebo-skillsagent (the hook scripts and the `init`/`recu`/`lint`
contract, both from the weebo-base template). Every file under `templates/` names its source in its
first comment. When a template and this text disagree, the template wins.

## 1. Layout

```
Taskfile.yml          includes + the fixed root tasks (init, recu, lint)
.tasks/<domain>.yaml  one file per domain (rust, ui, test, security, release …), tracked
.task/                Task's own checksum cache, gitignored. Never put Taskfiles here.
.hooks/*.sh           end-of-line and whitespace fixers, called by the pre-commit hook
.hooks/ignore         globs those fixers must not touch (vendored content)
mise.toml, mise.lock  every tool the repo calls, and their resolved versions/checksums
cog.toml              conventional commits + the git hooks
.env                  loaded by Task's `dotenv`, gitignored
```

## 2. Taskfile rules

| Rule | Why |
| --- | --- |
| Every task has a `desc` | `task --list` only shows tasks with one; it is the project's command index |
| Domain files are included with `flatten: true` | the name a task is declared with is the name everyone types, however files are split |
| A flattened file prefixes its own names (`ui:build`, `rust:lint`) when they could clash | Task refuses to load two flattened tasks with the same name |
| Toolbox files with generic names (`install`, `lock:gen`) are included without flatten | `mise:install` reads better than a bare `install` |
| Several commands with one purpose (tests, coverage, perf) get their own `.tasks/` file | the root file stays an index |
| Usage goes in the `desc`: `(usage: task x [VAR=default])` | the only help a user sees |

Fixed names every Weebo repo has, so hooks, CI and agents can call them blind:

| Task | Does | Owner |
| --- | --- | --- |
| `init` | `mise install`, `cog install-hook -a -o`, project setup | this skill |
| `recu` | regenerate tracked artefacts (OpenAPI spec, client); run by pre-commit | this skill |
| `lint` | every linter/format check in the repo; run by pre-commit | this skill |
| `mise:install`, `mise:lock:gen`, `mise:lock:update` | tools and lock | this skill |
| `test`, `coverage-check`, … | | `weebo-testing` |
| `audit` | CVE scan of deps and images | `weebo-security` |
| `release` | cocogitto bump | `weebo-release` |

Details: [references/taskfile.md](references/taskfile.md).

## 3. mise

mise is the only package manager: no `apt`, `dnf`, `curl | sh`, `cargo install` or `npm -g`.
`mise search <name>`, then `mise use <name>` (it writes `mise.toml`), then `task mise:lock:gen`.
Pin to an exact version when CI pins the same tool (gitleaks, a GitHub Action's `version`) or when a
CVE fix is not yet in mise's index; otherwise `latest`, and keep the WHY comment beside the pin.
Backends, the lock and the CDE caveats: [references/mise.md](references/mise.md).

## 4. Git hooks

cocogitto owns both hooks; `task init` installs them from `cog.toml`. Conventional Commits are
enforced by `cog verify` on commit-msg. Pre-commit runs `task recu`, `git add -u`, `task lint`,
`git add -u`, the two `.hooks/` fixers, then `gitleaks git --pre-commit --staged`. Details and
failure modes: [references/hooks.md](references/hooks.md).

An agent never commits: Weebo commits are signed and signed-off by the user. Write the message to a
file, check it with `cog verify --file <file>`, and hand the user the `git commit -S -s -F <file>`
command. The message ends with a `Co-Authored-By:` trailer naming the agent as co-author.

## 5. Rules

- **A missing tool is a `mise use`**, after `mise search`, asked of the user first.
- **The hook calls the task.** Never inline a linter in `cog.toml`; add it to `lint`.
- **`recu` output is tracked**, so pre-commit re-stages it; a generated file that drifts shows up in review.
- **`.tasks/` is tracked, `.task/` is not.** Scratch files a task writes go under `.task/`.
- **Never `--no-verify`.** Fix what the hook reports.

## Templates

```
templates/
├── Taskfile.yml           includes, dotenv, init / recu / lint
├── .tasks/rust.yaml       build, check, clean:stale, rust:lint, fmt (flattened)
├── .tasks/mise.yaml       install, lock:gen, lock:update (prefixed)
├── mise.toml              core tools + one example per backend
├── cog.toml               tag_prefix + the two hooks (bump config: weebo-release)
├── .hooks/end-of-line.sh
├── .hooks/whitespace-fixer.sh
├── .hooks/ignore
└── .gitignore
```

Copy them, `chmod +x .hooks/*.sh`, run `task init`. Replace the Rust lines when the stack differs.
