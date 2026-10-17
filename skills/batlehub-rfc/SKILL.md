---
name: batlehub-rfc
description: >
  Write, revise or land an RFC (design document) in the BatleHub house style,
  with mandatory security, supply-chain, privacy and operability blocks. Use
  when the user asks for an RFC or design doc, a "bis" of an existing RFC, to
  resolve an RFC's open questions, to threat-model a proposed change, to move
  an RFC to In review / Accepted / Implemented / Rejected, or to check one is
  ready for sign-off. Not for bug fixes, changelog entries or a roadmap alone.
user-invocable: true
argument-hint: "[new|revise|land|review] [title or NNNN]"
---

An RFC is a change worth arguing about *before* it is written, and the document
is what the argument is had against. The form lives in two files beside this
one, pointed at rather than repeated, because a third copy would drift:

- `template.md` — sections, header table, status vocabulary, what to delete.
  Read it in full before `new`.
- `concerns.md` — the questions behind the mandatory blocks (§7 to §11), and
  the table saying which ones need full treatment for which kind of change.

If the repo has its own template (`docs/internal/0000-rfc-template.md`) and
`task rfc:*` tasks, as batlehub/batlehub does, use those for the mechanics and
this skill for the rules; where the repo template lacks §8 to §11 of ours, add
them.

## 1. Before writing a line

**Verify against the source, not memory or the roadmap.** A roadmap entry is a
hypothesis written without the code open; BatleHub's RFC 0024 found two of its
claims wrong within the hour.

- Read the code the change touches and quote it: file, function, line. For a
  protocol, read the real client's source and name its version.
- Probe the real thing (`curl -sI`, a `HEAD` on the real files) for sizes,
  types, status codes, redirects. Numbers come from probes, not estimates.
- Read the nearest sibling RFC and reuse its vocabulary.
- Grep for every type, function and file the RFC names. Inventing symbols the
  codebase does not have makes it hard to review and harder to build.
- **Map the attack surface before designing**: list the inputs that become
  attacker-controlled, the boundaries crossed, and the data that moves. The
  design is then written to that map, not the map retrofitted to the design.
- Run `task audit` (or the stack's audit) before proposing a dependency, and
  read its advisories, licence and maintenance health.

## 2. `new` — scaffold, fill, check

- Number: one past the highest that ever existed in `docs/rfc/`, zero-padded to
  4, never reused. `task rfc:new TITLE=… SETTLES=…` when the repo has it,
  otherwise copy `template.md` to `docs/rfc/NNNN-<slug>.md` by hand and add it
  to whatever index the docs keep.
- Fill top to bottom, **delete every HTML comment**. Sections that do not apply
  are deleted and the numbering closed up — except the mandatory blocks (§4).
- `Co-author` is the model that wrote it with the user. `Touches` lists the
  components and trees actually edited. `Risk` is the highest rating among
  §7 to §10.

## 3. What no gate checks

- **Motivations are numbered and falsifiable**, each naming the code path or
  workflow that hurts today.
- **Non-goals are the most useful section.** What a reasonable reader would
  assume is in scope, one clause each on why not.
- **Before / after shows, prose tells, and it is drawn**: a config or client
  snippet and a mermaid diagram of the two paths, marking the request that is
  refused, rewritten or newly trusted.
- **Behaviour rules state the uninteresting case**: nothing blocked, the
  default upstream, an option absent as distinct from empty.
- **Validation splits hard errors from warnings**, and says where the operator
  sees each warning. A log line nobody reads is not an answer.
- **The design names the invariant**: "because X, Y cannot happen here".
- **Every security claim names its enforcement**: the function that checks it
  and the test that proves the bypass is refused. "Only admins can do this" is
  a claim; "`require_admin` in `routes/policy.rs` gates it, and
  `policy_denies_reader` proves it" is a design.
- **Defaults are safe**: a new option defaults to the closed, private, off
  side; a default that is not the safe one says why in §4.1.
- **Security controls fail closed** unless §10.2 argues otherwise, explicitly.
- **A "deliberately untouched" list** for what a reviewer will go looking for.
- **Alternatives are rejected on a concrete cost.** The literal reading of the
  roadmap entry is usually one of them; "do nothing" often is too.
- **Decisions keep their numbers**: moved from *Still open* to *Resolved* with
  the decision in bold and a one-line rationale. If it went against the draft's
  recommendation, say so.
- **Phases each leave the tree green**, and a security control never lands in a
  later phase than the surface it protects.
- **Word count**: past ~4 000 words, mark it as reference material the way the
  repo's docs do. Prose wraps at 80 columns; tables and code are exempt.

## 4. The mandatory blocks

Five blocks exist in **every** RFC, including a *bis*:

| Block | Holds |
| --- | --- |
| §6.P Proof against a real client | numbered observations a real consumer will produce |
| §7 Security | rating, threat model with STRIDE rows, authn/z, input, secrets, abuse |
| §8 Supply chain | every new dependency, image, action or tool, with CVE and licence status |
| §9 Data and privacy | every new datum: personal or not, where, how long, who reads it |
| §10 Operability | metrics, audit trail, alerts, failure direction, capacity |

§11 (accessibility, i18n, compliance, licensing, docs) is one line per concern,
each either handled or ruled out with its reason.

**An RFC missing any of them is not valid.** It stays `Draft`, is not signed
off, and `land` has nothing to measure against. "This change has no security
dimension" (or no new dependency, no new data, no client surface) is a valid
answer **inside** the block, with its reason — never a reason to delete it. The
two are indistinguishable afterwards: a missing block does not say whether the
author decided nothing applied or never asked. The written answer is a claim a
reviewer can refuse; that is why it is written.

Distrust your own judgement on a change that *feels* harmless. A log line can
leak a token, a metric label can carry an email, a "read-only" endpoint can
enumerate tenants, and a docs change can publish an internal hostname.

### 4.1 §6.P, the proof

Name the real consumer — package manager, browser, CLI, operator workflow — its
pinned version, and numbered **observations**, not intentions: the command run,
what crossed the wire, the fact that settles it (exit code, the client's own
error text quoted, a request that was *not* made, a counter that moved). The
floor includes one success and one **refusal**: what must be denied, denied,
with proof the refused bytes were never fetched. In BatleHub, map the change to
its heavy suite (`tests/heavy/authz.sh` for grants and tokens, `airgap.sh`,
`quarantine.sh`, `soak.sh`, …).

When no client can observe the change, keep the heading and write three
sentences: what a client would have seen had it a surface, why it has none, and
which suite carries the regression signal instead.

### 4.2 §7, security

Start with the rating. Draw the trust boundaries (mermaid), then one STRIDE row
per real threat with its mitigation, the code that enforces it and the residual
risk with who accepted it. §14 then carries **one negative test per row**: the
bypass attempted, refused. New parsers get a fuzz target; how each test layer is
written and run is the `weebo-testing` skill.

## 5. Diagrams

Two to four mermaid diagrams in §5 beside the before/after and the trust
boundaries. `flowchart` for a decision, `sequenceDiagram` for a request
crossing components, `graph` for wiring; each with a sentence saying what it
proves. Wrap labels in double quotes, `<br/>` for line breaks, and inside a
label use `#91;` `#93;` `#123;` `#125;` for brackets and braces — but the braces
that make a decision node (`B{"blocked?"}`) are structure, not label. Run the
repo's mermaid check if it has one; a broken fence can build green and render an
error box.

## 6. What else changes with an RFC

| When | What |
| --- | --- |
| Any header edit | the RFC index / sidebar (generated where the repo generates it) |
| It covers a roadmap entry | the entry gains `(RFC NNNN, path, status)` and moves with it |
| It corrects a claim elsewhere | that page, in the same change |
| It adds a repo convention | `CLAUDE.md` (one paragraph) and the contributing docs |
| It adds a dependency | `task audit` stays green; licence policy updated |
| It adds an API | the generated spec (utoipa) and the generated client (HeyApi) |

## 7. `review` — is it ready?

Read it as the reviewer would, and report in this order:

1. Each mandatory block (§6.P, §7, §8, §9, §10): present with content, present
   with a reasoned "none", or **missing**. Any missing → not valid, say so first.
2. Security claims without an enforcement point or a negative test.
3. Defaults that are not the safe side, and controls that fail open, without
   their argument.
4. Questions still open.
5. Everything else from §3 that is weak.

## 8. `revise` — questions and status

- Resolve a question by moving it up with its number and the decision; never
  delete it.
- **No status move past `Draft` while a mandatory block is missing** or *Still
  open* is non-empty. Adding a block after sign-off inverts the order the rule
  protects.
- Vocabulary: `Draft`, `In review`, `Accepted`, `Implemented`, `Rejected`,
  `Superseded by NNNN`. Files never move or get renumbered.
- Reopening a settled RFC is a *bis*, not an edit beyond a pointer in its
  header.
- A security finding raised in review that changes the design is a resolved
  question with its number, not a silent edit to §7.

## 9. `land` — when the implementation ships

- Append `## 17. Revision against the tree (YYYY-MM-DD)`: what shipped and every
  point where it differs from the text, each with its reason. Do not rewrite the
  earlier sections; the delta is the record.
- Say what the proof observed, not what the RFC predicted.
- For each §7 threat row: the test that now proves it, or the row that turned
  out not to hold and what replaced it. For §8: the versions that actually
  shipped and `task audit`'s result.
- Status `Implemented` with the landing dates; `Touches` and `Risk` updated;
  roadmap ticked; the index regenerated.

## 10. Finish

Run the repo's docs gates if it has them and report the last line. Then tell the
user, in this order: the number and path; the state of each mandatory block
(cases, reasoned none, or missing — missing first and loudly); the security
rating and the highest residual risk; what is still open; every other file
touched; what was verified against real code or a real client versus assumed.
Do not commit: the user signs.

---

Derived from `.claude/skills/rfc` and `docs/internal/0000-rfc-template.md` in
[batlehub/batlehub](https://github.com/batlehub/batlehub) at `2336695`
(Apache-2.0), extended with the security, supply-chain, privacy and operability
blocks.
