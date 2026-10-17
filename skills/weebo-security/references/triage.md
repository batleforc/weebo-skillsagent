# CVE triage and the exception policy

Source: batlehub@20879e5 (`deny.toml`, `.trivyignore.yaml`, `.gitleaksignore`, `gitleaks.toml`,
`.tasks/security.yaml`, `docs/contributing/security-scanning.md`).

## The stance

**No suppressions.** `.cargo/audit.toml` and `deny.toml` (`[advisories] ignore = []`) stay empty.
Nothing is ignored because it is inconvenient, only because there is no version to move to and the
vulnerable code is shown not to be reachable.

## Triage ladder: take the first rung that works

| # | Situation | Fix |
| --- | --- | --- |
| 0 | Is the advisory real? | Check `withdrawn` on the OSV record (`https://api.osv.dev/v1/vulns/<id>`). A withdrawn advisory is not chased **and not allowlisted**; gate on severity so it costs nothing. |
| 1 | A fixed version exists | Upgrade (`cargo update -p <crate>`, bump the parent, `pnpm up`). If that creates a duplicate crate, **the upgrade stands** and the `skip` is added in the same commit. Never downgrade to remove a duplicate. |
| 2 | The vulnerable crate comes in through a feature you don't need | Drop the feature (`default-features = false`), write the reason next to the declaration in `Cargo.toml`, and add the crate to `[bans].deny` so turning the feature back on fails CI. Batlehub closed RUSTSEC-2026-0258 (`h2 <0.4`, no backport) this way by turning off actix-web's `http2`. |
| 3 | The crate is unmaintained but a drop-in exists | Path-patch it (`[patch.crates-io]` → `patches/<crate>`, a shim re-exporting the replacement) and `deny` the bad crate. |
| 4 | The CVE is inside a **prebuilt third-party binary** the image copies in | Pin a `.trivyignore.yaml` entry to that one path, with `expired_at`, the reason the code is unreachable here, and the command that tells you whether the next upstream release retires it. Add the matching VEX statement ([sbom-vex.md](sbom-vex.md)). |
| 5 | A frontend advisory inside a package that ships a **prebuilt bundle** | An `overrides:` pin is a false green: it moves the lockfile and not the shipped bytes. Bump the parent, or `pnpm audit --ignore GHSA-…` with a dated comment saying who checked and whether the package reaches the bundle at all. One entry per advisory. |
| 6 | Nothing above applies | Escalate. Do not add an ignore to make CI green. |

Before deciding an advisory is unfixable, check whether a feature can be dropped (rung 2): it is
the fix most often missed.

## Every exception carries

| Field | Where it goes |
| --- | --- |
| Advisory id | the entry itself (`id:`, `--ignore GHSA-…`, deny `name`) |
| Scope | one path (Trivy `paths:`), one crate/version range, one fingerprint (gitleaks) — never a directory or a blanket rule |
| Why it doesn't apply here | a comment on the entry, in prose: which code path, why unreachable |
| Who checked, and when | a date in the comment ("Checked 2026-09-18") |
| Expiry | `expired_at:` in `.trivyignore.yaml`: the gate reopens on its own |
| What retires it | the upstream release/issue, and the command to check it before renewing |

The expiry lives in exactly one place (`.trivyignore.yaml`, the gate). The VEX document does not
repeat it: a date with two homes has one that is wrong. Renewing an entry means re-running the
check, not bumping the date. The upstream release that closes it **deletes** the entry.

## Severity bar

One bar everywhere: **fixable HIGH and CRITICAL block**. `pnpm audit --audit-level high`,
`postmortem --fail-on-vuln high`, Trivy `--severity HIGH,CRITICAL --ignore-unfixed`. cargo audit and
cargo deny block on any RUSTSEC vulnerability. A MODERATE finding below the bar is not suppressed
(there is nothing to suppress until the gate reports it); note it next to the entry that will retire
it if you know one.

## Scanner false positives are not dependency exceptions

The no-suppression stance is about dependency advisories. A static-analysis rule that is wrong about
this code gets one narrow, version-controlled entry with its reasoning inline:

- **gitleaks**: fix the fixture in the tree (rename it so it stops looking like a token). For
  history you cannot rewrite: one `.gitleaksignore` fingerprint (`commit:file:rule:line`), or a
  `regexes` value in `gitleaks.toml`. Prefer the value over a path: it survives renames, and a real
  credential in the same file is still flagged. An inline `gitleaks:allow` does not fix history.
- **Semgrep**: a path pattern in `.semgrepignore` per fixture family. `// nosemgrep` is silently
  ignored for the generic secrets rules.
- **CodeQL**: dismiss per alert in Code scanning and write a dated triage note. No repo-wide
  `query-filters`: turning off a query to quiet a few alerts trades a real control for a green check.

Re-run the scanner after adding an entry: a suppression that parses but does not apply looks
exactly like one that works.

## "Use the latest version that has no CVE" (house rule)

- `mise.toml` tools are `"latest"` unless pinned for a reason written next to the pin (gitleaks,
  to match CI; a version ahead of mise's index when a CVE fix is out). See `weebo-taskfile-mise`.
- Dependabot (`templates/.github/dependabot.yaml`) proposes bumps daily for cargo, npm and
  github-actions, with a 7-day `cooldown`. Security updates skip the cooldown.
- A `latest` tool with a finding and no newer release has **no fix to apply**. Build it from a
  pinned commit through mise's `go:` backend (batlehub does this for `helm-docs`), or drop it if
  nothing in the repo runs it.
- A newer version that carries a known CVE is not "latest" for our purposes: stay on the newest
  clean one and record why in a comment next to the pin.

## A vulnerability in our own code

Follow `SECURITY.md`: private report through GitHub's private vulnerability reporting, never an
issue; fix in the advisory's private fork; ship in one release carrying the GHSA (with a CVE
requested from GitHub), a `### Security` changelog entry, and a VEX statement (`fixed` for the new
release, `affected` with an "upgrade to" `action_statement` for the previous one). Publish the
advisory once the fixed artefacts are up. Release mechanics: `weebo-release`.
