---
name: weebo-good-practice
description: House coding practice for Weebo projects (Rust backend + TS frontend) — resilient, sober and secure code. No `unwrap`/`expect`/panic/unchecked indexing or arithmetic outside tests unless justified by a `#[expect(..., reason = "…")]`, typed errors, no ignored `Result`, no `unsafe` without encapsulation; strict TS with no `!`, `any` or unchecked casts; eco-design rules (RGESN, GR491, Opquast) from the Institut du Numérique Responsable's skill-nr; smallest change via ponytail. Use when writing, fixing or reviewing any Rust or TS code, when setting up a crate's lints or a tsconfig, when an `unwrap`, `unsafe`, `as`, `!` or `allow` attribute is added, or when a change adds a dependency, a loop over unbounded data, a cache or a query.
---

# Weebo good practice

Three axes, each with one source of truth:

| Axis | Source | Applied by |
| --- | --- | --- |
| **Resilient + secure** | [ANSSI Rust guide](https://github.com/ANSSI-FR/rust-guide) (rule IDs cited), extended with current clippy/rustc mechanisms | [references/rust.md](references/rust.md), [references/typescript.md](references/typescript.md), the lints in `templates/` |
| **Sober (eco-design)** | [skill-nr](https://github.com/Institut-du-Numerique-Responsable/skill-nr) (RGESN, GR491, Opquast, RGAA) | `references/nr/rules/`, the `eco-check` and `accessibilite-check` agents (item `nr-check`) |
| **Sober (build)** | ours | the Cargo profiles in [references/cargo.md](references/cargo.md) |
| **Minimal** | ponytail | the `ponytail` skill (in `base`), `ponytail-review` |

## 1. While writing code

1. **Invoke the `ponytail` skill** if it is not already active: the smallest complete change comes first; every rule below applies to the code that is left.
2. **Read the rule files for the languages you touch**, once per session, only those:

   | Touching | Read |
   | --- | --- |
   | `*.rs` | [references/rust.md](references/rust.md) + [nr/rules/ecoconception-rust.md](references/nr/rules/ecoconception-rust.md) |
   | `*.ts`, `*.vue`, `*.js` | [references/typescript.md](references/typescript.md) + [nr/rules/ecoconception-javascript.md](references/nr/rules/ecoconception-javascript.md) |
   | UI (HTML, CSS, components) | + [nr/rules/ecoconception-frontend.md](references/nr/rules/ecoconception-frontend.md), [nr/rules/qualite-web-opquast.md](references/nr/rules/qualite-web-opquast.md) |
   | SQL, migrations, queries | [nr/rules/ecoconception-sql.md](references/nr/rules/ecoconception-sql.md) |
   | an endpoint, a job, storage | [nr/rules/ecoconception-backend.md](references/nr/rules/ecoconception-backend.md) |
   | anything | [nr/rules/numerique-responsable.md](references/nr/rules/numerique-responsable.md) |

   Other languages: `references/nr/rules/ecoconception-<lang>.md`. The criteria they cite
   (`GR491_Backend_4`, `RGESN 4.2`, `Opquast n°124`) are in `references/nr/referentiels/`.

## 2. The non-negotiables

- **No panic on an expected error path.** Outside tests, `unwrap`, `expect`, `panic!`, `todo!`,
  `unimplemented!`, `[i]` indexing and overflowing `+ - *` are denied by lint. A real invariant
  keeps its panic, but says why, where the linter checks it:
  ```rust
  #[expect(clippy::expect_used, reason = "the regex is a literal, covered by test `pattern_compiles`")]
  static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]+$").expect("literal regex"));
  ```
  `#[expect]` (not `#[allow]`) fails the build once the panic site is gone, so the
  justification cannot outlive its code. A bare `allow`, or one without `reason`, is denied.
- **No ignored error.** No `let _ = fallible();`, no `.ok()` to discard, no `catch {}` swallowing.
  Handle, propagate with context, or log and say in a comment why dropping it is safe.
- **Typed errors at module boundaries** (`thiserror`); `anyhow` only in binaries and tests.
- **Validate at the trust boundary** (HTTP body, file, env, CLI arg, IPC), once, into a type that
  cannot hold an invalid value. Inside, trust the type.
- **No `unsafe`** (`unsafe_code = "deny"`). FFI lifts it per item with a reasoned `#[expect]` and
  wraps every `unsafe` behind a safe API with a `// SAFETY:` comment.
- **TS: no `any`, no `!`, no `as` (except `as const`), no floating promise**, `strict` +
  `noUncheckedIndexedAccess` on.
- **Bounded by default**: every cache, channel, queue, retry loop, page and request body has a
  limit and a timeout. Unbounded is a bug, not a feature.

## 3. Setting up a project

Rust: copy [templates/lints.toml](templates/lints.toml) into the root `Cargo.toml`, add
`[lints] workspace = true` to every member crate, and copy [templates/clippy.toml](templates/clippy.toml)
to the root (it lets tests unwrap), and add the profiles from [references/cargo.md](references/cargo.md)
so `target/` stays small. `task lint` (`cargo clippy --workspace --all-targets -- -D warnings`,
from `weebo-taskfile-mise`) is then the gate; nothing new to run. Existing code that fails: fix it,
or add one `#[expect(..., reason = "legacy, tracked in #<issue>")]` per site, never a crate-wide allow.

TS: the `tsconfig` and lint rules in [references/typescript.md §1](references/typescript.md).

## 4. Before handing over a change

1. `task lint` and `task test` pass.
2. Run the **`eco-check`** agent on the diff; on UI changes, also **`accessibilite-check`**.
   Fix what they find or say why not.
3. `grep -rn 'expect(clippy::\|SAFETY:' <changed files>`: every new justification is one you would
   defend in review. Name each in the hand-over.

## Sources and refresh

- `references/nr/` and the `nr-check` agents are vendored from skill-nr (CC BY-SA 4.0, full text in
  `LICENSES/skill-nr.txt`) through `copies` in `vendor.toml`. Never edit them: `task skills:sync
  SOURCE=skill-nr` refreshes them, `task skills:sync:check` reports drift.
- `references/rust.md` derives from the ANSSI guide at commit `3f9e2e2` (Licence Ouverte 2.0). The
  guide stops at the rules; the lint mechanisms (`#[expect]`, `reason`, `[workspace.lints]`) are ours.
