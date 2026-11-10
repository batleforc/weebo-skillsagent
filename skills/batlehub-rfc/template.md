<!--
HOW TO USE THIS TEMPLATE

Copy this file to docs/rfc/NNNN-<slug>.md (or run `task rfc:new` where the
repo has it), fill it top to bottom, and delete every HTML comment, this one
included. What is left must read as a document, not a filled-in form.

- NNNN is one past the highest number that has ever existed, zero-padded to 4.
  Numbers are never reused, even by a rejected or withdrawn RFC.
- The slug describes the change, not the solution:
  `0002-artifact-retention-policies`, not `0002-add-a-cron-job`.
- Section numbers are fixed, so §7 is Security in every RFC and references to
  it hold. A section or subsection that does not apply keeps its heading and
  one sentence on why: never a bare "N/A", never deleted. Only §17 waits for
  the implementation to ship.
- The MANDATORY blocks: §6.P (proof), §7 (security), §8 (supply chain), §9
  (data and privacy), §10 (operability) and §11 (other concerns). "None
  applies" is a valid answer inside them when it says why; a missing block is
  not, because a month later nobody can tell a decision from an omission.
- Size each block to the change: concerns.md says which ones get full
  treatment. The rest are one sentence, not tables of "n/a" rows:
      ## 8. Supply chain
      No new dependency, image, action or tool; `task audit` is unchanged.
- An RFC is for a change worth arguing about *before* it is written: new
  user-facing surface, a cross-component refactor, a security-relevant default,
  a new dependency or data flow, or anything expensive to undo. A bug fix is
  not an RFC.

STATUS VOCABULARY
  Draft            — being written; a mandatory block may still be missing
  In review        — every mandatory block written; open questions are what
                     the review settles
  Accepted         — Still open empty, no blocker; implementation may start
  Implemented      — merged; link the PRs/commits in the header
  Rejected         — decided against; keep the file, it records why
  Superseded by NNNN
-->

# RFC NNNN — <Title>

| Field       | Value                                                        |
| ----------- | ------------------------------------------------------------ |
| Status      | Draft                                                         |
| Short       | <How this is listed: a few words, no number>                  |
| Settles     | <One line: what this settles>                                 |
| Author      | Name <email>                                                  |
| Co-author   | —                                                             |
| Created     | YYYY-MM-DD                                                    |
| Supersedes  | —                                                             |
| Touches     | `crates/…`, `ui`, `helm`, `Containerfile`, docs               |
| Risk        | <low / medium / high — the highest rating in §7 to §10>       |

---

## 1. Summary

<!--
Three to six sentences. What changes, for whom, and what it looks like from the
outside. A reader who stops here should be able to describe the feature to
someone else. Show before telling: a config block, a CLI call, a client snippet.
-->

### Before / after

```text
# today

# with this RFC
```

```mermaid
flowchart LR
    subgraph T["today"]
        C1["client"] --> U1["upstream"]
    end
    subgraph W["with this RFC"]
        C2["client"] --> P["this component"] --> U2["upstream"]
    end
```

<!-- One sentence under the diagram: what it proves. Mark the request that is
refused, rewritten or newly trusted. -->

---

## 2. Motivation

<!--
Numbered, strongest first, each falsifiable: it names the code path or workflow
that hurts today. "It would be cleaner" is not a motivation.
-->

1. **<Problem>.** <Why it hurts today, with the specific code path or workflow.>
2. …

---

## 3. Goals / non-goals

**Goals**

- <One capability per bullet, phrased as an outcome, not an implementation.>

**Non-goals**

<!-- The most useful section. Include what a reasonable reader would assume is
in scope, one clause each on why not. -->

- <Thing deliberately not done, and one clause on why.>

---

## 4. User-facing design

### 4.1 Configuration

```toml
[section]
option = "value"    # what it does
```

- <Defaults, and what "absent" means as distinct from "empty".>
- <Which defaults are security-relevant, and why the default is the safe one.>

### 4.2 Behaviour rules

- <Normalisation, precedence, and what happens in the uninteresting case.>

### 4.3 Validation

Rejected at startup / on input:

| Condition | Rationale |
| --- | --- |
| <what> | <why this must fail rather than degrade> |

Warnings, and where the operator actually sees them:

| Condition | Behaviour | Surfaced in |
| --- | --- | --- |
| <what> | <what happens instead of failing> | <admin page, metric, startup log> |

### 4.4 API surface

<!-- If no API changes, one sentence saying so. Every endpoint added or changed: method, path,
auth required, who may call it, rate limit. In Rust the spec is generated by
utoipa — name the handler; on the front the client is regenerated with HeyApi. -->

| Method | Path | Auth | Who may call | Limit |
| --- | --- | --- | --- | --- |
| | | | | |

### 4.5 Mocks

<!-- REQUIRED when the change alters anything a person sees: web UI, TUI, CLI
output, prompts, error or help text. When it does not, one sentence: "No
visible surface: <why>." One mock per screen or command, each in the
states that matter: nominal, empty, error (the real message text), and the
refusal from §6.P. Text surfaces (CLI, TUI) are drawn as the exact output in a
`text` fence. A web UI is a wireframe in a `text` fence, or an HTML mock built
with the batlehub-design skill at docs/rfc/assets/NNNN-<screen>.html, linked
here. Under each mock: one sentence on what it decides (layout, wording,
order). A mock is a proposal; §17 records what shipped. -->

**<Screen or command> — <state>**

```text
$ tool command --flag
✔ done in 1.2s
```

<What this mock decides.>

---

## 5. Architecture

<!-- Which layer does what. Name the existing types and functions extended.
Two to four mermaid diagrams; each carries a sentence saying what it proves.
State the invariant: "because X, Y cannot happen here". -->

### 5.1 <Mechanism>

```mermaid
flowchart TD
    A["input"] --> B{"decision?"}
    B -->|yes| C["path taken"]
    B -->|no| D["path not taken"]
```

---

## 6. Detailed design

### 6.1 `<component>`

- <Change, with the concrete symbol or file.>

### 6.P Proof against a real client

<!--
MANDATORY. The last subsection of §6. The real consumer (package manager,
browser, CLI, operator) that will prove this works, named before it is built:
the suite, the client and its pinned version, and numbered OBSERVATIONS — the
command run, what crossed the wire, the fact that settles it (exit code, the
client's own error text quoted, a request that was NOT made, a metric that
moved). At least one case is a refusal: the thing that must be denied, denied.

If no real client can observe the change, keep the heading and write three
sentences: what a client would have observed if it had a surface, why it has
none, and which suite carries the regression signal instead.
-->

<Suite, client and pinning — or the three sentences.>

1. <Command run> — <what was requested, what answered, what settles it.>

**Deliberately untouched**, so reviewers do not go looking:

- `<path>` — <why it looks relevant but is not.>

---

## 7. Security

<!--
MANDATORY. Rate it (low / medium / high) in the first line. Every subsection
that does not apply is replaced by one sentence saying why, inside this
section — never deleted silently. Checklists: concerns.md §1–§3.
-->

**Rating:** <low/medium/high> — <one line why>.

### 7.1 Threat model

<!-- Assets worth protecting, actors, and every trust boundary the change adds
or moves. Name which inputs are attacker-controlled. -->

```mermaid
flowchart LR
    A["untrusted: client"] -- "boundary 1" --> B["this component"]
    B -- "boundary 2" --> C["trusted: storage"]
```

| # | Threat (STRIDE) | Where | Mitigation | Residual risk |
| --- | --- | --- | --- | --- |
| T1 | <Spoofing / Tampering / Repudiation / Info disclosure / DoS / Elevation> | <boundary> | <control, and the code that enforces it> | <accepted, and by whom> |

### 7.2 Authentication and authorisation

- <New surface: authenticated or not. Who may do what; default-deny or not;
  what a bypassed check gains an attacker — "nothing they could not already
  do" is a fine answer, with the reason.>

### 7.3 Input handling and injection

- <Parsing of untrusted input: paths (traversal), URLs (SSRF), archives (zip
  slip, bombs), templates, SQL, shell, headers. The limit on each size.>

### 7.4 Secrets and cryptography

- <New secrets: where they live, how they rotate, that they never reach logs,
  URLs or the client. Crypto: which primitive, which library, no home-made.>

### 7.5 Abuse and resource exhaustion

- <Rate limits, quotas, timeouts, max sizes, what an unauthenticated caller can
  make the server do and at what cost.>

### 7.6 Existing defences that still hold

- <Controls the change relies on and does not weaken.>

---

## 8. Supply chain

<!-- MANDATORY. Checklist: concerns.md §4. -->

| Dependency / image / tool | Version | Why this one | Licence | Known CVEs (`task audit`) | Maintenance |
| --- | --- | --- | --- | --- | --- |
| <crate, package, base image, action> | <latest without CVE> | <what we would write otherwise> | <SPDX> | <none / ID and why acceptable> | <last release, maintainers> |

- **Build and release**: <Containerfile changes (multi-stage, hardened variant),
  CI permissions, pinned actions, SBOM / signature impact — or "unchanged".>
- **Updates**: <how the new dependency is kept current (updatecli, renovate).>

---

## 9. Data and privacy

<!-- MANDATORY. Checklist: concerns.md §5. "No personal or new data is stored
or transmitted" is a valid answer when it is true and says why. -->

| Data | Personal? | Where stored | Retention | Who can read it | Leaves the instance? |
| --- | --- | --- | --- | --- | --- |
| <field / log line / metric label> | <no / yes: what> | | | | |

- <Lawful basis and erasure path when personal data is involved; what appears
  in logs, traces and metric labels; encryption at rest and in transit.>

---

## 10. Operability

<!-- MANDATORY. Checklist: concerns.md §6–§7. -->

### 10.1 Observability

- **Metrics**: <new counters/histograms, names and labels (no unbounded
  cardinality, no personal data in labels).>
- **Logs and audit trail**: <what is logged at which level; which actions
  write an audit event — every security-relevant decision does.>
- **Alerts**: <what pages someone, or "none, because…".>

### 10.2 Failure modes

| Failure | Detected by | Behaviour (fail open / closed) | Recovery |
| --- | --- | --- | --- |
| <dependency down, disk full, bad config> | | <and why that direction> | |

### 10.3 Performance and capacity

- <Budget: latency, memory, storage growth per unit of use, with the numbers
  measured or the probe that will measure them. Cost if it runs on paid infra.>

---

## 11. Other concerns

<!-- Each line: applies (and where it is handled) or does not (and why). Delete
a line only by writing why it does not apply. Checklist: concerns.md §8. -->

- **Accessibility**: <UI changes meet WCAG 2.2 AA — keyboard, focus, contrast,
  reduced motion — or "no UI".>
- **Internationalisation**: <new strings in every locale the product ships.>
- **Compliance**: <CRA, NIS2/DORA, ISO 27001, SOC 2 controls touched, or none.>
- **Licensing**: <our licence obligations change or not.>
- **Documentation**: <pages in docs/ written or corrected in the same change.>

---

## 12. Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| <approach> | <the concrete cost, not "it is worse"> |

---

## 13. Rollout and compatibility

- **Default behaviour** when the feature is not configured.
- **Migration**: config, schema, data — and whether it is reversible.
- **Operator prerequisites**: DNS, certificates, infrastructure, credentials.
- **Rollback**: what it takes to undo, what stays persisted, and whether a
  rollback re-opens any risk closed in §7.
- **Feature flag / staged rollout**, if any.

---

## 14. Test plan

- **Unit** (`<path>`): <cases>.
- **Integration** (`<path>`): <cases>.
- **Proof**: §6.P. <!-- written even when the answer is none: "**Proof**: none, §6.P" -->
- **Security** (`<path>`): <one negative test per T-row in §7.1 — the bypass
  attempted, refused; fuzz targets for new parsers.>
- **Audit**: `task audit` green with the new dependencies.
- **Existing suites** that must pass unchanged: <which, and what they prove>.

---

## 15. Decisions and open questions

<!-- Start with everything under "Still open". Resolved questions move up with
their number, the decision in bold and a one-line rationale; never deleted.
In review once §6.P to §11 are all written; Accepted once Still open is also
empty. -->

### Resolved

| # | Question | Decision |
| --- | --- | --- |
| 1 | <question> | **<decision>.** <one-line rationale.> |

### Still open

1. <Question, with the trade-off stated and a recommendation if you have one.>

---

## 16. Implementation phases

<!-- Ordered, each leaving the tree green. Say which phase is useful alone. A
security control never lands in a later phase than the surface it protects. -->

| Phase | Content |
| --- | --- |
| 1 | <scope> |

---

## 17. Revision against the tree (YYYY-MM-DD)

<!-- Added by `land` when the implementation ships; leave this heading out
until then. What shipped and every point where it differs from the text above,
each with its reason. The earlier sections are not rewritten: this delta is the
record. -->
