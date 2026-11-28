---
name: weebo-health
description: >
  The weebo-agent health scorecard (RFC 0001 §5.3 of batleforc/weebo-update) —
  each row of `missions/health/check.sh`, how it is measured, its weight, the
  house skill that owns the rule, the row states and how the score is computed,
  and how the weekly issue and the fleet summary are published. Use when reading
  or acting on a "weebo-agent:health" issue, fixing a red row in a repo, adding
  or reweighting a scorecard row, or when the health or conform missions are
  involved.
---

# Weebo health

The `health` mission runs every Monday on each `fleet.yaml` repo. `check.sh` is
deterministic: it runs in the cloned repo and prints JSON. The model only writes a three-line
trend, and only when the score moved on an `allow_llm` repo. The source is
`missions/health/check.sh` in [weebo-update](https://github.com/batleforc/weebo-update);
when this table and the script disagree, the script wins.

## 1. Rows

| Id | Checks | Weight | Fix with |
| --- | --- | --- | --- |
| `taskfile-desc` | a Taskfile exists and every task of it and of `.tasks/*.yaml` has `desc` (`internal` tasks exempt) | 10 | `weebo-taskfile-mise` |
| `audit` | `task audit` exists and exits 0 within `AUDIT_TIMEOUT` (900 s) | 15 | `weebo-security` |
| `deps` | `.tasks/deps.yaml` exists | 5 | `weebo-deps` |
| `mise-lock` | `mise.lock` next to `mise.toml` (n/a without mise) | 5 | `weebo-taskfile-mise` |
| `renovate` | a Renovate config, and no other update bot (Dependabot, updatecli) | 10 | `weebo-deps` |
| `renovate-overrides` | local Renovate `packageRules` carry a `description` | 0 (warn) | `weebo-deps` |
| `actions-pinned` | every `uses:` in `.github/workflows` pinned by 40-char SHA | 10 | `weebo-security` |
| `containerfile` | the first Containerfile or Dockerfile found has a `.hardened` twin (n/a without one) | 5 | `weebo-containerfile` |
| `ci-main` | the last completed run on the default branch succeeded | 15 | the repo's CI |
| `ruleset` | a default-branch ruleset with a `pull_request` rule, squash only | 15 | GitHub settings (plan H9) |
| `oldest-pr` | the oldest open PR is younger than `STALE_PR_DAYS` (30) | 5 | triage |
| `last-release` | tag and age of the last release | 0 (info) | `weebo-release` |
| `forgejo-main` | Forgejo mirror = GitHub main | 0 (n/a until the mirror, M7.1) | — |

The local rows read the clone; the remote rows (`ci-main` onwards) read the GitHub API,
anonymously or with a read token.

## 2. States and score

States: `ok`, `fail`, `warn`, `unknown`, `n/a`.

**score = weight of `ok` rows ÷ weight of `ok` + `fail` rows**, as a percentage. `unknown`
(API unreadable, rate limit, a field that needs a token) and `n/a` rows are left out: an
outage never lowers a score, and a repo without an image is not penalised for it. The
squash-only part of `ruleset` needs the App token, so it is `unknown` without one.

## 3. Publication

- **Per repo**: one open issue labelled `weebo-agent:health`, opened by `weebo-agent[bot]`,
  upserted every week and **edited only when the body changes**, so a quiet week makes no
  notification. The body carries no date.
- **Fleet summary**: one issue on weebo-update, after the pass, worst score first, listing the
  repos that produced no report. It is the only place the run date appears.

## 4. Changing a row

1. Edit `check.sh`: one `row <id> <label> <state> <detail> <weight>` call per outcome. Keep it
   deterministic, read-only and offline-safe (an unreadable source is `unknown`, not `fail`).
2. Add the row's cases to `tests/health/check.bats`.
3. Update this table and the owning skill if the rule is new.
4. A weight change moves every repo's score: say so in the commit, since the trend line will
   report it.

Related: `weebo-mission` (mission schema, outcomes, `task mission:run`).
