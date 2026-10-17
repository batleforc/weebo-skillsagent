---
name: wska
description: >
  Install, check, update or add Claude Code skills, agents, plugins and MCP
  servers with wska, the Weebo skills linker, instead of copying files into
  .claude/ by hand or running `claude plugin install` / `claude mcp add`
  directly. Use whenever a skill the task needs is missing or broken, when
  setting up a project's agent tooling, when asked to "install the skills",
  "add skill X", "set up bmad/graphify/rtk", "update the skills", or when
  creating or editing a skill that should live in weebo-skillsagent.
---

`wska` links items (skills, agents, installer-driven tools) from the
weebo-skillsagent repo into a Claude Code target. It is the only supported way
to put Weebo skills into a project. Full reference: `docs/wska.md` in that repo.

## Hard rules

- **Never** copy, write or symlink anything into `.claude/skills/` or
  `.claude/agents/` by hand, and never run `claude plugin install` or
  `claude mcp add` yourself. A one-off copy goes stale and `wska` cannot see
  or update it. If something must be installed by a command, it becomes an
  `[installers.<item>]` entry in the repo's `vendor.toml`.
- **To add or drop something, use `wska add` / `wska remove`**, which keep
  the rest of what `.claude/wska.toml` records. `install --profile/--item` is declarative: it *removes*
  every wska link not named, so use it only to replace the whole selection.
- Running installers executes commands (`npx`, `uv tool install`, …). Tell the
  user which profiles and installers you are about to run and get a yes first.
- Never edit a skill through a path the harness gave you without knowing it is
  a symlink into the items repo: the edit is live in *every* project. Edit in
  the items repo on purpose, or not at all.

## Finding the binary

1. `wska` on PATH — use it.
2. Inside weebo-skillsagent itself: `task skills:<command>` (wraps
   `cargo run -p wska`).
3. Neither: propose `mise use -g github:batleforc/weebo-skillsagent` to the
   user. Do not install it without asking.

## Install skills into the current project

```bash
wska list --recorded         # the recorded selection (.claude/wska.toml), profiles expanded
wska install --run-installers   # bare: re-applies wska.toml (fresh clone, after a pull)

wska status                  # what is linked now (Linked / MissingFromTarget / ...)
wska list --profiles         # profiles: what each includes and lists
wska list                    # every item, its kind, and the profiles listing it ("-" = --item only)
wska add --profile planning --item weebo-techdoc --dry-run         # check the plan
wska add --profile planning --item weebo-techdoc --run-installers  # apply, appends to wska.toml
wska remove --item weebo-techdoc                                   # drop a recorded name, unlink what only it brought
wska doctor                  # every item: tools present, SKILL.md usable
```

Pick profiles from the task: `base` always; `design`/`frontend` for UI work
(`batlehub-design` is the house design system); `planning` for RFCs and bmad;
`docs` for `docs/techdoc/`; `review` for judge gates; `mcp` for MCP servers.
A skill no profile lists goes in with `--item <name>`; do not create a
profile for one project's need. `--profile all` installs everything.

Then:

- Make sure `.claude/skills/` is gitignored in that project — the links are
  absolute paths into a local clone — and that `.claude/wska.toml` is not:
  committed, it lets every machine re-apply the selection with a bare install.
- Newly linked skills are read at session start: tell the user to start a new
  session (or `/clear`) if the skill does not show up.
- A `warn` or a `doctor` `missing:` line is not a blocker for linking, but the
  skill will fail when run. Offer to install the missing tool via mise.
- `refuse … a real file or directory is already there`: nothing was written.
  Show the user the path; they decide whether to move or delete it.

`--scope user` targets `~/.claude/` instead of the project. Use it only when
the user asks for a machine-wide install.

## Update

```bash
wska update --run-installers   # git pull --ff-only, then re-apply .claude/wska.toml
```

Pass `--profile`/`--item` only to change the selection, with the same
declarative rule: name everything the target should keep.

## A skill you need does not exist

Add it to weebo-skillsagent rather than inlining it in the project:

1. `skills/<name>/SKILL.md` with frontmatter `name` and `description`
   (`wska doctor` rejects a SKILL.md missing either). For a vendored upstream
   skill, add a `[sources.*]` entry to `vendor.toml` and run `wska sync`; for
   a git-hosted one, `[external.<name>]`; for a command, `[installers.<name>]`.
2. Add the item to a profile in `profiles.toml`.
3. List the tools it needs under `[requires]` in `vendor.toml`.
4. Add its row to the items table in `readme.md`.
5. `wska doctor`, then `wska add --item <name>` (or a bare `wska install` if a
   recorded profile now lists it).

Commits in that repo are signed by the user: write the Conventional Commits
message to a file and hand over the command, never commit yourself.
