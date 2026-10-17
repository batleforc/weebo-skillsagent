# Unit and integration tests

## 1. Unit tests

- Inline `#[cfg(test)] mod tests { use super::*; … }` at the bottom of the module.
- When the suite outgrows the file, move it to a sibling `tests.rs` (`registry/pypi/tests.rs`
  declared by `#[cfg(test)] mod tests;` in `registry/pypi/mod.rs`).
- Mock HTTP upstreams with `mockito` (a dev-dependency), never with a live network call.
- Async tests: `#[tokio::test]`.
- Test names describe behaviour: `publish_traversal_version_returns_400`, not `test_publish_3`.

## 2. In-process integration tests

`<crate>/tests/<feature>.rs` builds the real app in-process (for actix:
`actix_web::test::init_service`) over in-memory adapters, and drives it through HTTP requests.

- One file per feature or per variant (`local_npm_registry.rs`, `proxy_pypi_edge_cases.rs`).
- Shared setup lives in `tests/common/mod.rs`. Each file starts with `mod common; use common::*;`.
- `common/mod.rs` starts with `#![allow(dead_code, unused_imports)]`: every test binary compiles its
  own copy and uses only a subset of it.
- Prefer in-memory adapters to real backends. A real backend belongs in §3.
- CLI binaries: `cli/tests/integration.rs` runs the built binary as a subprocess
  (`env!("CARGO_BIN_EXE_<name>")`).

## 3. Real-backend tests (Postgres, Redis, S3)

These live in `crates/adapters/tests/pg_*.rs`, `redis_*.rs` and `s3_*.rs`. They opt in through an
env var, so `task test` works on a machine without containers. The skip is always visible:

```rust
fn db_url() -> Option<String> {
    let url = std::env::var("DATABASE_URL").ok();
    if url.is_none() {
        eprintln!("SKIPPED: {} — DATABASE_URL unset", module_path!());
    }
    url
}

#[tokio::test]
async fn set_and_get_round_trip() {
    let Some(url) = db_url() else { return };
    // …
}
```

- Isolate each test with a unique key prefix or schema (an `AtomicU64` counter), so tests can run
  in parallel against one database.
- One task per backend starts the container and sets the variable (`task test:pg-cache`). The
  `coverage` task starts every backend, so the gate counts these tests.
- CI sets the variables (service containers), so nothing skips there. A CI log that contains
  `SKIPPED:` is a bug.
- Opt-in features stay behind cargo features (`--features storage-s3`).

## 4. Gate tests

When an enum's variants each need something in several places (a route, a perf scenario, an authz
row, an OpenAPI entry), write one test that loops over every variant and fails, naming the variant,
when something is missing. Batlehub's examples are `registry_kind_coverage.rs`,
`soak_kind_coverage.rs`, `authz_matrix.rs`, `openapi_contract.rs` and `profile_route_coverage.rs`.

```rust
#[test]
fn every_kind_has_a_perf_registry() {
    let missing: Vec<_> = RegistryKind::iter()          // strum::EnumIter
        .filter(|k| !PERF_REGISTRIES.contains(&k.as_str()))
        .collect();
    assert!(missing.is_empty(), "kinds with no perf registry: {missing:?}");
}
```

With an exhaustive `match` and no `_` arm, the compiler forces the next variant into the gate.

## 5. Security negatives

For every refusal the code makes (path traversal, SSRF, authz denial, oversize body), write a test
that attempts the bypass and asserts the refusal: the status code, and that no side effect
happened. Batlehub's rule: each new registry kind gets a
`<kind>_publish_traversal_version_returns_400` test.

## 6. Frontend

- vitest + `@vue/test-utils`, `environment: "jsdom"`, `testTimeout: 20_000` (CI runners are
  slower than the 5 s default assumes).
- Co-locate `Foo.test.ts` beside `Foo.vue`. Under `src/components/`, one test covers its whole
  directory.
- Generated API clients (HeyApi) are mocked at the axios instance, not per function.
