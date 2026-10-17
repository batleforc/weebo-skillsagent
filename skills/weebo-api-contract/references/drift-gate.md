# Drift gates

Three layers, each catching what the others cannot.

## 1. Contract test (`crates/web/tests/openapi_contract.rs`)

Template: `templates/crates/web/tests/openapi_contract.rs`. Runs under `task test`
(see `weebo-testing`).

| Test | Reads | Fails when |
| --- | --- | --- |
| `every_success_response_declares_a_body` | the generated spec (`openapi_spec()`) | a `200`/`201` has no schema (a `$ref` counts as one) |
| `the_spec_is_not_empty` | the generated spec | fewer than `MIN_ROUTES` paths: route collection broke and the test above passes vacuously |
| `every_declared_success_status_is_one_the_handler_can_actually_send` | this crate's `src/**/*.rs` | the 2xx in `#[utoipa::path]` and the literal `HttpResponse::Ok/Created` / `StatusCode::OK/CREATED` in the handler body disagree, in either direction |

- The body check walks the spec, not the source, so it cannot be satisfied by a comment.
- The status check is a source lint because nothing generates that fact. It skips a handler whose
  status comes from a variable rather than guessing; it also asserts `checked >= MIN_ROUTES`,
  because its first draft in batlehub scanned nothing and passed.
- `MIN_ROUTES` is `1` in the template; batlehub uses `100`. Raise it as the API grows, a little under
  the real count.

## 2. Spectral (`task openapi:lint`, `.github/workflows/openapi-lint.yaml`)

Lints the **committed** `ui/openapi.json` with `spectral:oas` plus the OWASP API Top 10 ruleset,
failing on warnings (`--fail-severity` set to `warn`): the spec stays at zero findings, and every exception is a rule turned off in
`.spectral.yaml` with its reason beside it. It catches duplicate `operationId`s, a missing summary
(a custom error rule), a missing `info.contact`, and a write endpoint without `security`.

- Run via `npx --yes --ignore-scripts @stoplight/spectral-cli@6.17.0`, pinned, not a devDependency:
  its tree carries advisories that would fail `ui:audit`.
- The workflow triggers on changes to `ui/openapi.json`, `.spectral.yaml` and itself. That is why
  committing the regenerated spec with an API change matters: it is what makes the lint run.
- Batlehub's `task security` aggregate calls `openapi:lint` (see `weebo-security`).

## 3. CI regenerates the client every time

The client is gitignored, so it cannot be stale in CI. The front-test and Sonar jobs rebuild both
halves from source before testing:

```yaml
- name: Install dependencies
  run: pnpm install --frozen-lockfile --ignore-scripts
- name: Generate OpenAPI spec
  working-directory: .
  run: cargo run -p my-server -- dump-spec > ui/openapi.json
- name: Generate TypeScript client
  run: pnpm run generate
- name: Run tests with coverage
  run: pnpm run coverage
```

This job needs the Rust toolchain (`dtolnay/rust-toolchain` + `Swatinem/rust-cache`) as well as
pnpm/node. A UI change that no longer type-checks against the current API fails here.

The rendered-design job generates from the committed spec without building the server: it measures
pages, not the contract.

## 4. What no gate covers in batlehub

Nothing fails when the committed `ui/openapi.json` lags the code: CI regenerates it in the front jobs
but never compares. Spectral then lints a stale file. Running `task generate` and committing
`ui/openapi.json` with every API change is a review convention there, not a check. See SKILL.md §5
for the shape of a check if a project wants one.
