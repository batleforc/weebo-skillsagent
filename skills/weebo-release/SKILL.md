---
name: weebo-release
description: House release practice for Weebo projects, modelled on batlehub and weebo-skillsagent — the cocogitto bump (`cog bump`, `chore(version): vX.Y.Z` commit, `v` tag, pre/post-bump hooks), the hand-written or generated changelog, `task version:set`/`version:check` that keep Cargo.toml, Cargo.lock, package.json, the OpenAPI spec and Chart.yaml on one version, and the tag-triggered GitHub release workflow (CLI build matrix, image, Helm chart, SBOMs, keyless cosign signing, checksums, draft release). Use when preparing a release, adding a release workflow or the version tasks to a project, shipping a new artifact, or when a version is out of step somewhere.
---

# Weebo release practice

The reference implementations are [batlehub](https://github.com/batleforc/batlehub) (full:
server, CLI, image, chart) and weebo-skillsagent (one CLI, `wska`). Templates under `templates/`
are snapshots of batlehub at commit `20879e5` or of weebo-skillsagent, trimmed. When a template
and this text disagree, the template wins: it is what ran.

## 1. Agents never release

The bump commit is signed by the user, the tag is the release trigger, and a push publishes. So an
agent **never** runs `cog bump` (without `--dry-run`), `git tag`, `git commit`, `git push`, or
`gh release edit --draft=false`. The agent prepares the change (changelog entry, `version:set`
wiring, workflow) and prints the commands for the user:

```bash
cog bump --auto --dry-run                 # agent may run this: shows the next version
task version:check                        # agent may run this
cog bump --auto                           # user: runs version:set, commits, tags v1.2.0
git push --atomic origin HEAD v1.2.0      # user, unless post_bump_hooks already pushes
```

## 2. The flow

| Step | Who | What happens |
| --- | --- | --- |
| Work lands | agent + user | conventional commits; hand-written changelog: the `[Unreleased]` entry is written in the same change |
| Cut | user | `cog bump` → `pre_bump_hooks` (`task version:set VERSION=x.y.z`) → commit `chore(version): vX.Y.Z` → lightweight tag `vX.Y.Z` |
| Push | user | the commit and the tag, atomically (`--follow-tags` skips lightweight tags) |
| Build | CI | `on: push: tags: ["v*"]` builds and uploads every artifact; perf report on the same tag |
| Publish | user | batlehub: reviews the **draft** release and publishes it; wska: the release is public at once |
| Consume | users | `mise use github:<owner>/<repo>`, `ghcr.io/<repo>:x.y.z`, `oci://ghcr.io/<repo>/charts` |

Details: [references/cocogitto.md](references/cocogitto.md),
[references/release-workflow.md](references/release-workflow.md).

## 3. Taskfile contract

```yaml
includes:
  release: { taskfile: .tasks/release.yaml, flatten: true }
```

| Task | Does |
| --- | --- |
| `version:set VERSION=1.2.0` | writes the bare version into Cargo.toml `[workspace.package]`, Cargo.lock, `ui/package.json`, `ui/openapi.json`, `Chart.yaml` `version` + `appVersion`, regenerates the chart README; then `version:check` |
| `version:check` | fails, naming the file, if any declaration disagrees with Cargo.toml |

Drop the lines for files the project does not have. A single-crate repo can use the two-line
`pre_bump_hooks` instead ([cocogitto.md §4](references/cocogitto.md)).

## 4. Rules

- **One version, written by one task.** Never hand-edit a version in one file; run
  `version:set`. The tag and the files it points at must agree.
- **The tag is `v`-prefixed, the files are not.** `{{version}}` is bare; `version:set` refuses a `v`.
- **Pick one changelog mode** and say it in `cog.toml`: hand-written Keep a Changelog
  (`disable_changelog = true`, batlehub) or generated from commits (weebo-skillsagent).
- **Release assets are reproducible from the tag:** `--locked` builds, chart `--version` and
  `--app-version` taken from the tag, actions pinned by SHA.
- **Sign and attest by digest, keylessly** (cosign + `actions/attest-*` with `id-token: write`).
- **Many artifacts → draft release.** Every job uploads to one draft; `checksums.txt` is made last
  from what is actually there; a person publishes.
- **The perf report never blocks a release.** Separate workflow on the same tag, attached when done.

## 5. Templates

```
templates/
├── cog.toml                                   release keys; merge into weebo-taskfile-mise's cog.toml
├── .tasks/release.yaml                        version:set, version:check
└── .github/workflows/
    ├── release-cli-only.yaml                  one CLI, mise github: asset names (weebo-skillsagent)
    └── release.yaml                           server + CLI matrix + image + chart + checksums (batlehub)
```

Replace every `my-server` / `my-cli` / `my-crate` placeholder.

## 6. Related skills

`weebo-taskfile-mise` (cog hooks, Taskfile layout, mise), `weebo-testing` (the perf workflow and
report attached to releases), `weebo-security` (`task audit`, Trivy, SBOM/VEX policy),
`weebo-containerfile` (the images built here), `weebo-helm` (the chart pushed here),
`weebo-api-contract` (the OpenAPI spec whose `info.version` is bumped here).
