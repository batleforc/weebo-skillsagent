---
name: weebo-security
description: House security and supply-chain practice for Weebo projects (Rust backend + TS frontend + container image), modelled on batlehub — the mandatory `task audit` (cargo audit, cargo deny, pnpm audit, Trivy on the image), the CI scanners and their daily schedules (gitleaks, Semgrep, CodeQL, postmortem, image rebuild+scan), CycloneDX SBOMs, OpenVEX, CVE triage and the exception policy (no suppressions; expiry and justification when there is no fix). Use when setting up `task audit` or security CI, when an audit, scanner or Dependabot PR reports a CVE or secret, when adding or bumping a dependency, or when an ignore/allowlist entry is proposed.
---

# Weebo security practice

The reference implementation is [batlehub](https://github.com/batleforc/batlehub). Every file
under `templates/` is a snapshot of batlehub at commit `20879e5`, trimmed to what any project can
copy. When a template and this text disagree, the template wins.

## 1. `task audit` (mandatory, every project)

The house rule: `task audit` checks the dependencies **and** the image for CVEs.

| Task | Runs | Fails on |
| --- | --- | --- |
| `audit` | the four below | any of them |
| `audit:rust` | `cargo audit` | any RUSTSEC vulnerability |
| `deny` | `cargo deny check` (`deny.toml`) | advisory, yanked, banned crate, unlisted duplicate, non-allowlisted license, unknown source |
| `ui:audit` | `pnpm audit --audit-level high` in `ui/` | high/critical |
| `audit:image` | `trivy image --severity HIGH,CRITICAL --ignore-unfixed --exit-code 1 --ignorefile .trivyignore.yaml $IMAGE` | fixable high/critical |

`audit:image` needs `IMAGE=…`. There is no container engine in the CDE, so without one it prints
`SKIPPED: audit:image — IMAGE unset` on stderr and CI's `image-scan.yaml` is the gate. Drop
`ui:audit` or `audit:image` only when the project has no UI or no image.

Batlehub names cargo audit `audit` and puts the aggregate in `security`. The template renames it to
`audit:rust` so `task audit` meets the house rule.

`security` = `audit` + `postmortem` (supply-chain reputation, needs network) + `sbom:rust`.
Template: `templates/.tasks/security.yaml`, included with `flatten: true` like the other `.tasks/` files
(layout: `weebo-taskfile-mise`).

## 2. CI: what gates, and when

| Workflow | Tool | Trigger | Gate |
| --- | --- | --- | --- |
| `back-dep-audit.yaml` | cargo audit + cargo deny | Cargo files, **daily** | block |
| `dep-audit-frontend.yaml` | pnpm audit | lockfile, **daily** | block high/critical |
| `image-scan.yaml` | build + Trivy (SARIF, then gate) | Containerfile/lockfiles/ignores, **daily rebuild** | block fixable high/critical |
| `postmortem.yaml` | postmortem, one job per lockfile root | every push/PR, **daily** | block high vulns |
| `secret-scan.yaml` | gitleaks 8.30.1, full history | every push/PR, **daily** | block |
| `semgrep.yaml` | Semgrep `p/rust p/typescript p/secrets` (SARIF, then gate) | source files, weekly | block on ERROR |
| `codeql.yaml` | CodeQL rust + JS/TS | every push/PR, weekly | report |
| `dependabot.yaml` | cargo, npm, github-actions bumps | daily, 7-day cooldown | n/a |

The daily runs are the point: a CVE disclosed after the last commit trips CI the next morning with
nothing changed. Details, cron times, and the advisory-only scans batlehub adds on top
(`vuln-scan.yaml`, `mise-scan.yaml`): [references/scanners.md](references/scanners.md).

## 3. Rules

- **No suppressions.** `.cargo/audit.toml` and `deny.toml`'s `ignore` stay empty. Upgrade, drop the
  feature that pulls the crate in, or patch it out, then `[bans].deny` it so it cannot come back.
- **An exception has a scope, a reason, a date and an expiry.** Only for a CVE with no version to move
  to and code shown unreachable: one path (`.trivyignore.yaml`, `expired_at:`), one advisory
  (`pnpm audit --ignore GHSA-…` with a dated comment), one fingerprint (`.gitleaksignore`). Never a
  directory, never a blanket rule. Ladder: [references/triage.md](references/triage.md).
- **Security outranks tidiness.** If an upgrade that closes an advisory creates a duplicate crate,
  the upgrade stands and the `skip` lands in the same commit. Never downgrade.
- **Use the latest version that has no CVE.** Tools `latest` in `mise.toml`, Dependabot daily;
  any pin carries its reason in a comment.
- **One bar everywhere:** fixable HIGH/CRITICAL, with the same flags in the task and the workflow.
- **Report, then gate.** Upload SARIF with `if: always()` before the step that fails the job.
- **Pin actions by SHA** with the tag in a comment; pin gitleaks to the same version as `mise.toml`.
- **A withdrawn advisory is neither chased nor allowlisted.** Check `withdrawn` on its OSV record.

## 4. Conditional layers

| Trigger | Add | Reference |
| --- | --- | --- |
| The project ships a container image | `audit:image`, `image-scan.yaml`, `.trivyignore.yaml` | [scanners.md](references/scanners.md) |
| `.trivyignore.yaml` gets its first entry | `vex/my-project.openvex.json`, `.github/scripts/vex.py`, `task vex` in `security`, the `vex` job in `image-scan.yaml` | [sbom-vex.md](references/sbom-vex.md) |
| A release publishes artefacts | `sbom:rust` + a syft SBOM per image, attested at release (`weebo-release`) | [sbom-vex.md](references/sbom-vex.md) |
| A test needs a committed key or token-shaped fixture | a value `regexes` entry in `gitleaks.toml` and a path in `.semgrepignore` | [triage.md](references/triage.md) |
| The repo is public or has users | `SECURITY.md` (private reporting, targets, 90-day disclosure) | [triage.md](references/triage.md) |

## 5. Tooling

Add to `mise.toml` on top of what `weebo-taskfile-mise` already pins (cargo-audit, cargo-deny,
gitleaks 8.30.1):

```toml
trivy = "latest"                       # image / filesystem / SBOM CVE scanner
syft = "latest"                        # SBOM generation for built images
semgrep = "latest"                     # static analysis (Rust / TS / secrets rulesets)
"cargo:cargo-cyclonedx" = "latest"     # CycloneDX SBOM of the Rust workspace
"github:mlab-sh/postmortem" = "2.5.0"  # same version as postmortem.yaml's `version:`
```

## Templates

```
templates/
├── .tasks/security.yaml          audit, audit:rust, deny, ui:audit, audit:image, postmortem*, vex*, sbom:rust, security
├── deny.toml                     advisories, bans (skip/deny policy), licenses, sources
├── .cargo/audit.toml             no suppressions
├── gitleaks.toml                 default rules + value allowlist
├── .gitleaksignore               per-finding fingerprints for history only
├── .semgrepignore                semgrep.sarif + fixture paths
├── .trivyignore.yaml             empty, with one commented example entry
├── vex/my-project.openvex.json   one example statement, matching that entry
├── .github/scripts/vex.py        `check` / `render`
├── .github/dependabot.yaml
├── .github/workflows/{back-dep-audit,dep-audit-frontend,image-scan,postmortem,secret-scan,semgrep,codeql}.yaml
└── SECURITY.md
```

Replace every `my-server` / `my_crate` / `my-project` / `my-org` placeholder. Leave the rest as it
is unless the project gives a reason to change it.
