# Concern checklists

The questions behind the mandatory blocks of `template.md`. Answer every
question that applies **in the RFC**, not here. A question you skip is one a
reviewer asks. A "no" needs its reason: "no new surface, the handler reuses
`require_admin`" is an answer; "no" is not.

## When a block needs more than one sentence

| The RFC… | Full treatment of |
| --- | --- |
| adds or changes an endpoint, a route, a CLI flag reachable remotely | §7 (all), §10.1 |
| touches login, tokens, grants, roles, sessions | §7.1, §7.2, §7.4, §9 |
| parses something a user or upstream sends | §7.3, §7.5, fuzzing in §14 |
| fetches from a URL that is configurable or user-supplied | §7.3 (SSRF), §7.5 |
| adds a dependency, base image, CI action or tool | §8 |
| stores, logs or forwards anything about a person | §9 |
| adds a background job, cache, queue or new storage | §10.2, §10.3 |
| changes a default | §4.1, §7, §13 |
| changes the UI | §11 accessibility and i18n |

Everything not in that table still gets its one sentence.

## 1. Threat model

- What are the assets: credentials, artifacts, configuration, availability,
  audit history, users' personal data?
- Who are the actors: anonymous client, authenticated user, admin, upstream
  registry, another tenant, an operator with shell access, the CI?
- Where are the trust boundaries, and which does this change add, move or
  widen? Draw them in §7.1.
- Walk STRIDE per boundary: spoofing, tampering, repudiation, information
  disclosure, denial of service, elevation of privilege. One row per real
  threat; write "considered, not applicable because…" for the rest only if a
  reviewer would expect them.
- For every mitigation: which line of code enforces it, and which test proves
  the bypass is refused.
- Residual risk is a decision with an owner, not a silence.

## 2. Authentication, authorisation, sessions

- Is the new surface reachable without credentials? Should it be?
- Default deny: what does an absent grant mean?
- Is authorisation checked on the object, not only on the route (IDOR)?
- Does the check happen before any expensive or side-effecting work?
- Tokens: scope, lifetime, revocation, storage (hashed at rest?), and whether
  they can leak through URLs, referrers, redirects or logs.
- Cross-origin: CORS, CSRF on cookie-authenticated mutations, cookie flags.
- Multi-tenant: can one namespace or tenant observe another's existence?

## 3. Input, injection, secrets, crypto, abuse

- Path traversal and zip slip on anything that becomes a file path.
- SSRF on anything that becomes a URL; are private ranges and metadata
  endpoints refused, and redirects re-checked?
- Injection: SQL, shell, template, header (CRLF), log forging, YAML/TOML
  type confusion.
- Size, depth and count limits on every parsed input; decompression bombs.
- Secrets: never in the repo (gitleaks runs in pre-commit), never in an image
  layer, never in a log line, a metric label or an error returned to a client.
  Where they live, how they rotate, what happens when one leaks.
- Crypto: a vetted library and primitive, constant-time comparison for
  secrets, no custom schemes, TLS verification never disabled by default.
- Abuse: rate limits, quotas, timeouts, concurrency caps; the cost asymmetry
  between what an anonymous caller sends and what the server does.

## 4. Supply chain

- Is the dependency necessary, or is it a few lines we could own?
- Latest version without a known CVE (`task audit`: `cargo audit` /
  `cargo deny`, `pnpm audit`, `trivy` or `grype` for images).
- Licence compatible with ours; recorded in the dependency licence policy.
- Maintenance health: release recency, number of maintainers, open security
  advisories, typosquat check on the exact name.
- Transitive weight: how many new crates or packages it pulls in.
- Build: base image pinned by digest, multi-stage, hardened variant updated
  (`Containerfile.hardened`); CI actions pinned by SHA with least permissions.
- SBOM and signatures: does the release artifact change shape?
- Who updates it, and how (updatecli, renovate, by hand on audit failure)?

## 5. Data and privacy

- What new data is collected, derived, logged or forwarded?
- Is any of it personal (IP addresses, usernames, emails, user agents count)?
- Purpose and lawful basis; minimisation — can it be hashed, truncated, or not
  collected at all?
- Retention period and the mechanism that enforces it.
- Erasure and export: how a person's data is found and removed (GDPR art. 15,
  17).
- Does any of it leave the instance: telemetry, upstream requests carrying
  user identity, third-party services? Air-gapped deployments must still work.
- Encryption in transit and at rest; backups inherit the same retention?

## 6. Observability and audit

- Metrics: names follow the existing prefix; labels bounded (no package name,
  path or user as a label unless the cardinality is capped).
- Logs: level chosen for the operator who reads them; no secrets, no personal
  data beyond what §9 declares.
- Audit trail: every security decision (grant, deny, login, config change,
  policy override) writes an event with who, what, when, and outcome.
- Tracing: spans across new component boundaries.
- Alerts: what deserves to wake someone, with its threshold; dashboards
  updated.

## 7. Reliability, performance, cost

- Failure of each new dependency: fail open or fail closed, and why that
  direction is the safe one for this feature. Security checks fail closed.
- Timeouts and retries with backoff; no retry storm toward an upstream.
- Idempotency of anything retried.
- Capacity: storage growth per unit of use, memory per request, latency budget
  on the hot path — measured, or the probe that will measure it.
- Hot reload and multi-instance (HA) behaviour: shared state, locks, caches.
- Cost on paid infrastructure (egress, storage class, compute).
- Disaster recovery: is the new state in the backup, and restorable?

## 8. Everything else

- **Accessibility** (UI): WCAG 2.2 AA — keyboard path, visible focus, contrast
  in every theme, reduced motion, labels for assistive tech.
- **Internationalisation**: every new string in every shipped locale; no
  concatenated sentences; dates, numbers and plurals localised.
- **Compliance**: which control of CRA, NIS2/DORA, ISO 27001, SOC 2 or the
  project's own checklist this touches, and whether its evidence changes.
- **Licensing**: our own licence obligations, notices, attribution files.
- **Documentation**: the operator guide, the API reference (utoipa-generated
  for Rust services), the changelog entry, and any page whose claim this
  change makes false — corrected in the same change.
- **Deprecation**: what is removed, the warning period, and how users find out.
