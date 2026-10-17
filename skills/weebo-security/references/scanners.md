# Scanners: what runs, where, and how hard it gates

Source: batlehub@20879e5, `.github/workflows/*`, `.tasks/security.yaml`,
`docs/contributing/security-scanning.md`.

## The matrix

| Layer | Tool | Local | CI workflow | Trigger | Gate |
| --- | --- | --- | --- | --- | --- |
| Rust advisories | `cargo audit` (RUSTSEC) | `task audit:rust` | `back-dep-audit.yaml` | Cargo files, daily | block |
| Rust advisories + bans + licenses + sources | `cargo deny` (`deny.toml`) | `task deny` | `back-dep-audit.yaml` `deny` job ¹ | same | block |
| JS dependencies | `pnpm audit --audit-level high` | `task ui:audit` | `dep-audit-frontend.yaml` | lockfile, daily | block on high/critical |
| Container / OS layers | Trivy, fixable HIGH/CRITICAL | `task audit:image IMAGE=…` | `image-scan.yaml` | Containerfile/lockfiles, daily rebuild | block |
| Supply chain (reputation + vulns) | postmortem | `task postmortem` | `postmortem.yaml` | every push/PR, daily | block on high vulns |
| Secrets | gitleaks 8.30.1, full history | `GITLEAKS_CONFIG=$PWD/gitleaks.toml gitleaks git .` | `secret-scan.yaml` | every push/PR, daily | block |
| SAST | Semgrep `p/rust p/typescript p/secrets`, ERROR | `semgrep scan --config p/rust --config p/typescript` | `semgrep.yaml` | source files, weekly | block on ERROR |
| SAST | CodeQL (rust, javascript-typescript) | none | `codeql.yaml` | every push/PR, weekly | report (Security tab) |
| Lockfile CVEs, 2nd opinion | vuln-scan-action (vuln.mlab.sh) | none | `vuln-scan.yaml` | PR | **report-only** PR comment |
| Dev toolchain | `trivy rootfs` over mise installs | `task mise:cve` | `mise-scan.yaml` | mise files, weekly | budget (red when the count grows) |

¹ batlehub's docs place cargo-deny in `back-dep-audit.yaml`, but at 20879e5 that workflow runs only
`rustsec/audit-check`. The template adds the `deny` job so the documented gate exists.

The secret scan also runs in the pre-commit hook (see `weebo-taskfile-mise`). clippy `-D warnings`
and the coverage gate are `weebo-testing`'s.

## Schedules (UTC)

| Cron | What |
| --- | --- |
| `50 23 * * *` daily | cargo audit, pnpm audit, postmortem, gitleaks |
| `20 2 * * *` daily | image **rebuild** + Trivy |
| `50 23 * * 1` Monday | Semgrep |
| `50 23 * * 0` Sunday | CodeQL |
| `40 6 * * 1` Monday | mise toolchain scan |

The daily runs are the point: they turn a build-time snapshot into *future* CVE detection. A CVE
disclosed against a pinned dependency or a base layer after the last commit trips CI the next
morning with nothing in the repo changed. The image job rebuilds rather than rescanning an old
image, so a refreshed base layer is picked up too.

## Shapes worth copying

- **Report, then gate.** Semgrep, Trivy and postmortem run twice or soft-fail first: one pass writes
  SARIF and always uploads it (`if: always()`), a second pass (or a `Gate` step reading the exit
  code) fails the job. A hard failure before the upload would hide the findings from the Security tab.
- **Same flags locally and in CI.** `--severity HIGH,CRITICAL --ignore-unfixed` for Trivy,
  `--audit-level high` for pnpm, `--fail-on-vuln high` for postmortem: one bar everywhere.
- **Tool versions match.** gitleaks is pinned to the same version in `mise.toml` and the workflow; a
  drifting `latest` once failed CI on a fixture no contributor could reproduce.
- **No `paths:` filter where the verdict depends on live data** (postmortem, vuln-scan, CodeQL): an
  unchanged lockfile can become vulnerable without a commit, and a filtered required check never
  runs, so it stays pending forever instead of passing.
- **Actions pinned by SHA** with the tag in a trailing comment; Dependabot's `github-actions`
  ecosystem bumps them.

## Per-scanner notes

**cargo audit / cargo deny.** `cargo audit` knows RUSTSEC. `cargo deny` adds unmaintained and
yanked crates, `[bans].deny` (crates removed for a security reason must never come back),
`multiple-versions = "deny"` with every duplicate enumerated in `skip`, a permissive license
allowlist and crates.io-only sources. `[graph]` has no `targets` filter so advisories for the
Windows/macOS artefacts are still reported.

**pnpm audit.** Resolves from `pnpm-lock.yaml`, no install, no lifecycle scripts. The shipped UI is
audited **without** `--prod` because its build tooling produces what ships; a docs site that ships
nothing it builds gets `--prod` (batlehub's `docs:audit`, report-only postmortem for the same reason).

**Trivy (image).** `ignore-unfixed: true`: a finding with no fixed version has nothing to move to and
goes to triage, not to the gate. Suppressions live in `.trivyignore.yaml` only, each with
`expired_at` (see [triage.md](triage.md)). A second image built `FROM` the first is scanned and gated
separately so its own toolchain answers for its own CVEs. Image construction is `weebo-containerfile`'s.

**gitleaks.** The upstream binary, not `gitleaks/gitleaks-action` (it demands a paid licence for
organisation-owned repos and exits 1 having scanned nothing). `fetch-depth: 0` + `gitleaks git`
walks full history; `--redact` keeps values out of the public log.

**Semgrep.** `pipx install semgrep`, rulesets `p/rust p/typescript p/secrets`, `--severity ERROR`.
`.semgrepignore` must list `semgrep.sarif`, or the gate pass rescans the report the first pass wrote.

**CodeQL.** Report only. No `query-filters`: a standing alert on shipped code is dismissed per alert
in Code scanning, with the reasoning in a dated triage note in the docs. `paths-ignore` is for code
that never ships (a perf harness), never for code whose alerts are inconvenient.

**postmortem** (`github:mlab-sh/postmortem`). Scores each dependency's source repo (stars, age,
activity, archived, typosquat) and cross-checks vuln.mlab.sh. It catches the "no advisory yet" shape
of supply-chain risk that advisory scanners cannot. Reputation thresholds (`--max-risk` …) stay unset
until a baseline exists. Needs `GITHUB_TOKEN` locally and the `VULN_MLAB_TOKEN` secret in CI.

**vuln-scan.yaml (advisory).** Sends `Cargo.lock`, `mise.lock`, and a syft CycloneDX SBOM of each
pnpm root to vuln.mlab.sh and edits one PR comment in place. It never fails the build: on batlehub
four of its five findings were withdrawn advisories or a scanner artefact (it flags a crate that a
`[patch.crates-io]` path patch already replaced), and RUSTSEC findings come back `severity: unknown`.
Add it only alongside the blocking gates, never instead of one.

**mise-scan.yaml (budget).** The `mise.lock` lockfile scan reads only the `cargo:`/PyPI tools; every
tool installed as a release binary looks clean to it. `trivy rootfs` (not `fs`: only `rootfs` reads
the Go module list embedded in a binary) over the installed toolchain sees them. None of it ships,
so the gate is a budget file that goes red when the count **grows**; raising it is a commit with a
reason. Copy it only with its report scripts (`.github/scripts/mise_cve_report.py`,
`mise_repo_tools.sh` in batlehub).
