---
name: weebo-api-contract
description: House API contract chain for Weebo projects (Rust actix backend + TS frontend), modelled on batlehub — utoipa + utoipa-actix-web annotations with a self-hosted Scalar docs page, the `dump-spec` subcommand that exports the OpenAPI spec, the HeyApi (`@hey-api/openapi-ts`) axios client generated from it, and the drift gates (contract test, Spectral lint, CI regeneration). Use when adding or changing an HTTP endpoint or DTO, setting up the backend-to-frontend client generation in a new project, wiring `/scalar`, or when the generated client returns `unknown` or drifts from the server.
---

# Weebo API contract

The reference implementation is [batlehub](https://github.com/batleforc/batlehub). Every file under
`templates/` is a snapshot of batlehub at commit `20879e5`, trimmed to what any project can copy.
When a template and this text disagree, the template wins: it is what ran.

## 1. The chain

| Step | Where | Run with |
| --- | --- | --- |
| Annotate | `#[utoipa::path]` + actix route macro on each handler, `ToSchema` on each DTO | n/a |
| Collect | `collect_routes(cfg)` registers every handler once; `ApiDoc` holds info, tags, security | n/a |
| Serve docs | `/scalar`, self-hosted bundle, own CSP | `my-server --static-dir ui/dist` |
| Export | `my-server dump-spec > ui/openapi.json` (a clap subcommand, not a test or a build script) | `task dump-spec` |
| Generate | `@hey-api/openapi-ts` → `ui/src/client/` (gitignored) | `task ui:generate` |
| Both | dump then generate | `task generate` |
| Gate | contract test, Spectral, CI regenerates before every front job | `task test`, `task openapi:lint` |

Details: [references/backend.md](references/backend.md),
[references/client.md](references/client.md), [references/drift-gate.md](references/drift-gate.md).

## 2. What is tracked

- `ui/openapi.json` **is committed**: an API change shows up in review as a diff of the contract,
  and Spectral lints that file.
- `ui/src/client/` **is gitignored** (`.gitignore` and `ui/.gitignore`). Every CI job that needs it
  runs `pnpm run generate` first, so a stale client cannot exist in CI.

## 3. Rules

- **One route list.** The running server and `openapi_spec()` both call `collect_routes`. A route
  registered anywhere else is served but undocumented.
- **Every `200`/`201` names a body** (`body = T`). Without it HeyApi types the response `unknown`
  and the UI ends up hand-writing the DTO. Enforced by `tests/openapi_contract.rs`.
- **The declared 2xx is the one the handler constructs.** Enforced by the same test's source lint.
- **Every operation has a summary**: the `///` line above `#[utoipa::path]`. Spectral fails without it.
- **Handler fn names are unique crate-wide.** utoipa uses the fn name as `operationId`, which is the
  generated SDK function name. A duplicate gave batlehub `listPackages2`; `spectral:oas` catches it.
- **Write endpoints declare `security(("bearer_token" = []))`.** The OWASP ruleset fails a write
  operation without one.
- **No Scalar from a CDN.** The bundle is a devDependency, copied into the build, served same-origin
  under a strict CSP. Why: [references/backend.md §3](references/backend.md).
- **Axios, not fetch.** House rule (repo-root CLAUDE.md). Batlehub still ships `@hey-api/client-fetch`;
  the three differences are in [references/client.md §2](references/client.md).

## 4. Tooling

```toml
# mise.toml: node and pnpm only; everything else is a pinned package.json devDependency
node = "24"
pnpm = "latest"
```

| Package | Where | Why |
| --- | --- | --- |
| `utoipa = { version = "6", features = ["actix_extras", …] }` | Cargo workspace | annotations |
| `utoipa-actix-web = "0.2"` | Cargo | route collection from `cfg.service(...)` |
| `utoipa-scalar = { version = "0.4", features = ["actix-web"] }` | Cargo | `/scalar` service |
| `@hey-api/openapi-ts` | `ui` devDependency | generator (`"generate": "openapi-ts"`) |
| `axios` | `ui` dependency | runtime of the generated client |
| `@scalar/api-reference` | `ui` devDependency | bundle copied by `build/copy-scalar.mjs` |
| `@stoplight/spectral-cli@6.17.0` | `npx`, **not** a devDependency | its tree carries advisories that would fail `ui:audit` |

Keep `typescript` on `^6`: `@hey-api/openapi-ts` 0.99 crashes under TypeScript 7
(`Cannot read properties of undefined (reading 'AnyKeyword')`).

## 5. Not yet practised

Batlehub does none of these, so this skill gives no convention for them:

- **A committed-spec freshness check.** CI regenerates the spec for the front jobs, but nothing fails
  when the committed `ui/openapi.json` is older than the code. If a project wants one, copy the shape
  of batlehub's `ui:tokens:check`: run `task dump-spec`, then `git diff --exit-code ui/openapi.json`.
- **Breaking-change detection** between two specs (oasdiff and the like).

## Templates

```
templates/
├── crates/web/src/openapi.rs          ApiDoc, SecurityAddon, collect_routes, openapi_spec, scalar(), API_DOCS_CSP
├── crates/web/src/handlers/example.rs one annotated handler + DTO
├── crates/web/tests/openapi_contract.rs  the contract gate (body + status lint)
├── server/src/main.rs                 dump-spec subcommand, server wiring, /scalar CSP
├── ui/openapi-ts.config.ts            axios client into src/client
├── ui/src/clientInit.ts               baseURL + bearer token
├── ui/build/copy-scalar.mjs           self-host the Scalar bundle
├── .tasks/api.yaml                    dump-spec, ui:generate, generate, openapi:lint
├── .spectral.yaml                     OAS + OWASP ruleset, every exception justified
└── .github/workflows/openapi-lint.yaml
```

Replace every `my_crate` / `my-crate` / `my-server` placeholder. Raise `MIN_ROUTES` in the contract
test as the API grows. Related skills: `weebo-testing` (where the contract test runs, coverage),
`weebo-taskfile-mise` (Taskfile includes, mise), `weebo-security` (`task audit`, which
`openapi:lint` sits beside), `weebo-containerfile` (shipping `ui/dist` with the server).
