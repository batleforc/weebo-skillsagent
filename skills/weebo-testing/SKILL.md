---
name: weebo-testing
description: House test practice for Weebo projects (Rust backend + TS frontend), modelled on batlehub — unit and in-process integration tests, the 80% coverage gate, and the conditional layers (cargo-fuzz fuzz targets, k6 load scenarios, real-backend tests, heavy real-client e2e, gate tests) with the Taskfile contract and CI jobs that run them. Use when adding tests, setting up a project's test tooling, adding a parser, endpoint, backend or enum variant that needs coverage, or reviewing whether a change is tested.
---

# Weebo testing practice

The reference implementation is [batlehub](https://github.com/batleforc/batlehub). Every file
under `templates/` is a snapshot of batlehub at commit `20879e5`, trimmed to what any project can
copy. When a template and this text disagree, the template wins: it is what ran.

## 1. Required core (every project)

| Layer | Where | Run with |
| --- | --- | --- |
| Unit | inline `#[cfg(test)] mod tests`; a sibling `<module>/tests.rs` once the suite spans files | `task test` |
| Integration (in-process) | `<crate>/tests/<feature>.rs`, all sharing `tests/common/mod.rs` | `task test` |
| Frontend unit | vitest, co-located `X.test.ts` beside `X.vue`/`X.ts` | `task ui:test` |
| Coverage gate | cargo-llvm-cov, vitest v8; **80%** lines (and branches/functions/statements for the UI) | `task coverage-check`, `task ui:coverage` |

Details: [references/unit-integration.md](references/unit-integration.md),
[references/coverage.md](references/coverage.md).

## 2. Conditional layers: add one when its trigger is true

| Trigger | Layer | Reference |
| --- | --- | --- |
| The code parses or validates input it did not produce (paths, archives, manifests, URLs, tokens) | **Fuzz**: one cargo-fuzz target per parser | [fuzz.md](references/fuzz.md) |
| It serves HTTP | **Load**: k6 scenarios with thresholds | [k6.md](references/k6.md) |
| It talks to Postgres / Redis / S3 | **Real-backend tests**: opt-in by env var, loud skip | [unit-integration.md §3](references/unit-integration.md) |
| External tools consume it (package managers, IDEs, browsers) | **Heavy e2e**: bash suites driving the real client | [heavy-e2e.md](references/heavy-e2e.md) |
| An enum has variants that each need a route, a test or a scenario | **Gate test**: fails when a variant is missing from the matrix | [unit-integration.md §4](references/unit-integration.md) |

When a trigger is true and the layer is missing, adding it is part of the change.

## 3. Taskfile contract

These task names are fixed in every Weebo repo, so an agent can run tests anywhere without looking
first. Put them in `.tasks/*.yaml` and include each file with `flatten: true` so the names stay
unprefixed:

```yaml
includes:
  test: { taskfile: .tasks/test.yaml, flatten: true }
  coverage: { taskfile: .tasks/coverage.yaml, flatten: true }
  perf: { taskfile: .tasks/perf.yaml, flatten: true }
```

| Task | Does |
| --- | --- |
| `test` | `cargo test --workspace` |
| `test:all` | `test` and `ui:test` in parallel |
| `coverage` / `coverage-check` | HTML + lcov report / fail under 80% |
| `fuzz TARGET=… MAX_TIME=30` | fuzz one target locally |
| `fuzz:check` | every fuzz target compiles (the per-PR gate, no nightly) |
| `perf:run:<scenario>` / `perf:run:all` | one k6 scenario with thresholds enforced / all, report only |
| `perf:report:compare BASE=… [FAIL=1] [MARGIN=25]` | diff against a baseline run |

Every task has a `desc`. Templates: `templates/.tasks/{test,coverage,perf}.yaml`.

## 4. Tooling

Pin the tools in `mise.toml`, never install them by hand:

```toml
"cargo:cargo-llvm-cov" = "latest"
"cargo:cargo-fuzz" = "latest"   # if fuzzing
k6 = "latest"                   # if load testing
```

The nightly toolchain is the one exception: only `cargo +nightly fuzz` needs it, so it stays out of
`mise.toml` and the `fuzz` task calls it with `+nightly`.

## 5. Rules

- **A skip is never silent.** A test that needs a backend or tool which is absent prints
  `SKIPPED: <test> — <VAR> unset` on stderr and returns. CI sets the variable, so nothing skips there.
- **A gate that cannot fail is not a gate.** Fuzz targets live in their own workspace, which
  `cargo check --workspace` never sees, so `fuzz:check` runs on every PR.
- **Every security refusal gets a negative test** (the bypass attempted, then refused), next to
  the code that refuses.
- **The CI job calls the task**, or matches it line for line. A local run and a CI run of the same
  layer must not drift.

## 6. CI

`templates/.github/workflows/test.yaml` holds the unit, integration and coverage jobs plus the
two-tier fuzz job (compile on every PR; 60 s per target, nightly, with a cached and minimised corpus).
`templates/.github/workflows/perf.yaml` runs k6 when a release is published or when a PR is
labelled `perf`. Soak, breaking-point and profiling runs are described in
[k6.md](references/k6.md); add them when the project needs them.

## 7. Not yet practised

Batlehub uses none of these, so this skill gives no convention for them. If a project adopts one,
put it next to its sibling layer and add its task to the contract:

- **proptest**: inline beside the unit tests, for invariants over generated inputs (a cheaper
  cousin of a fuzz target).
- **criterion**: `benches/`; micro-benchmarks, where k6 measures the whole service.
- **insta**: snapshot tests for rendered documents; review snapshots with `cargo insta review`.
- **cargo-mutants**: to check that the tests catch a mutated line.

## Templates

```
templates/
├── .tasks/test.yaml            test, test:all, fuzz, fuzz:check
├── .tasks/coverage.yaml        coverage, coverage-check
├── .tasks/perf.yaml            perf:run:*, perf:report:compare
├── fuzz/Cargo.toml             separate cargo-fuzz workspace
├── fuzz/fuzz_targets/fuzz_example.rs
├── perf/k6/config.js           base URL + token from env
├── perf/k6/helpers.js
├── perf/k6/scenarios/01_warm_read.js
└── .github/workflows/{test,perf}.yaml
```

Replace every `my_crate` / `my-crate` / `my_parse` placeholder. Leave the rest as it is unless the
project gives a reason to change it.
