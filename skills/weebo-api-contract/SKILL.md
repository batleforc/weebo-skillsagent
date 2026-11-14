---
name: weebo-api-contract
description: House API contract chain for Weebo projects (any HTTP backend + TS frontend), modelled on batlehub (actix) and ProxyAuthK8S (axum) — the backend generates its OpenAPI spec from code (in Rust, utoipa with the integration for the chosen framework: actix-web, axum, …; elsewhere, the framework's own generator), a self-hosted Scalar docs page, the export of the spec (`dump-spec` subcommand or a `swaggergen` binary), the HeyApi (`@hey-api/openapi-ts`) axios client generated from it, and the drift gates (contract test, Spectral lint, CI regeneration). Use when adding or changing an HTTP endpoint or DTO, setting up the backend-to-frontend client generation in a new project, wiring `/scalar`, or when the generated client returns `unknown` or drifts from the server.
---

# Weebo API contract

The contract is framework-agnostic: **the backend produces its OpenAPI spec from its own code, the
spec is committed, the TS client is generated from it, and gates catch drift.** The backend language
and HTTP framework are the project's choice; this skill does not impose one.

Two reference implementations, one per Rust flavour; each template is a trimmed snapshot:

| Flavour | Reference | Templates |
| --- | --- | --- |
| actix-web | [batlehub](https://github.com/batleforc/batlehub) `20879e5` | `templates/actix/` |
| axum | [ProxyAuthK8S](https://github.com/batleforc/proxyAuthK8S) `8fe220b` (migrated from actix, RFC 0007) | `templates/axum/` |

Everything else under `templates/` (Scalar page, UI, tasks, Spectral, CI) is shared. Off Rust, keep
the chain and the rules and swap the backend pieces (§4). When a template and this text disagree,
the template wins: it is what ran.

## 1. The chain

| Step | Where | Run with |
| --- | --- | --- |
| Annotate | the stack's OpenAPI annotations on each handler and DTO (Rust: `#[utoipa::path]` + the framework's route, `ToSchema`) | n/a |
| Collect | one function registers every handler once, for both the server and the spec; `ApiDoc` holds info, tags, security | n/a |
| Serve docs | `/scalar`, self-hosted bundle, own CSP | `my-server --static-dir ui/dist` |
| Export | actix: `my-server dump-spec`; axum: `cargo run -p swaggergen`; either `> ui/openapi.json`, never a test or a build script | `task dump-spec` |
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

- **One route list.** The running server and `openapi_spec()` both call the same route-registration function (`collect_routes` on actix, `app_router` on axum). A route
  registered anywhere else is served but undocumented; on axum, a route that cannot go through
  `routes!` (a `{*path}` catch-all) is listed by hand in `ApiDoc`'s `paths(...)`.
- **Every `200`/`201` names a body** (`body = T`). Without it HeyApi types the response `unknown`
  and the UI ends up hand-writing the DTO. Enforced by `tests/openapi_contract.rs`; the axum version
  allows a `NO_BODY` exemption with its reason (an empty liveness `200`, a relayed body).
- **The declared 2xx is the one the handler constructs.** Enforced on actix by the same test's
  source lint; not yet practised on axum (§5).
- **Every operation has a summary**: the `///` line above `#[utoipa::path]`. Spectral fails without it.
- **Handler fn names are unique crate-wide** (or operation ids, outside utoipa). utoipa uses the fn name as `operationId`, which is the
  generated SDK function name. A duplicate gave batlehub `listPackages2`; `spectral:oas` catches it.
- **Write endpoints declare `security((<scheme> = []))`.** The OWASP ruleset fails a write
  operation without one. The scheme name is the project's choice (batlehub `bearer_token`,
  ProxyAuthK8S `bearer_auth`), as long as `SecurityAddon` and every handler use the same one.
- **No Scalar from a CDN.** The bundle is a devDependency, copied into the build, served same-origin
  under a strict CSP. Why: [references/backend.md §3](references/backend.md).
- **Axios, not fetch.** House rule (repo-root CLAUDE.md). Batlehub still ships `@hey-api/client-fetch`;
  the three differences are in [references/client.md §2](references/client.md).

## 4. Tooling

### Backend: per stack

The front half (HeyApi, Scalar bundle, Spectral) never changes. The backend half does:

| Stack | Spec from code | Route collection | `/scalar` |
| --- | --- | --- | --- |
| Rust + actix-web (`templates/actix/`) | `utoipa` 6, feature `actix_extras` | `utoipa-actix-web` 0.2 | `utoipa-scalar` 0.4, feature `actix-web` |
| Rust + axum (`templates/axum/`) | `utoipa` 6, feature `axum_extras` | `utoipa-axum` 0.3 (`OpenApiRouter`, `routes!(...)`) | `utoipa-scalar` 0.4, feature `axum` |
| Rust, other framework | `utoipa` (framework-free `#[utoipa::path]`) | list paths in `#[openapi(paths(...))]` and check it in the contract test | `utoipa-scalar` if it has the feature, else serve the custom HTML yourself |
| Not Rust | the framework's generator (FastAPI native, NestJS `@nestjs/swagger`, Hono `@hono/zod-openapi`, Go `huma`, …) | whatever keeps one route list | serve the self-hosted bundle with the same CSP (references/backend.md §3) |

Per framework: the route wiring, the `/scalar` CSP middleware, static files, the export, and the
contract test (actix adds a status source lint; axum adds `NO_BODY`). The Scalar page itself
(`templates/crates/web/src/scalar.rs`) is the same on both. Details:
[references/backend.md](references/backend.md). Everything in §2–§3 applies to every stack.

### Front and shared

```toml
# mise.toml: node and pnpm only; everything else is a pinned package.json devDependency
node = "24"
pnpm = "latest"
```

| Package | Where | Why |
| --- | --- | --- |
| `@hey-api/openapi-ts` | `ui` devDependency | generator (`"generate": "openapi-ts"`) |
| `axios` | `ui` dependency | runtime of the generated client |
| `@scalar/api-reference` | `ui` devDependency | bundle copied by `build/copy-scalar.mjs` |
| `@stoplight/spectral-cli@6.17.0` | `npx`, **not** a devDependency | its tree carries advisories that would fail `ui:audit` |

Keep `typescript` on `^6`: `@hey-api/openapi-ts` 0.99 crashes under TypeScript 7
(`Cannot read properties of undefined (reading 'AnyKeyword')`).

## 5. Not yet practised

Neither reference does these, so this skill gives no convention for them:

- **A committed-spec freshness check.** CI regenerates the spec for the front jobs, but nothing fails
  when the committed `ui/openapi.json` is older than the code. If a project wants one, copy the shape
  of batlehub's `ui:tokens:check`: run `task dump-spec`, then `git diff --exit-code ui/openapi.json`.
- **Breaking-change detection** between two specs (oasdiff and the like).
- **The declared-vs-sent status lint on axum.** The actix lint matches `HttpResponse::Ok/Created`;
  an axum handler often returns `Json(..)` alone (a `200`), which a literal match cannot see.

## Templates

Copy the shared files plus one flavour directory, merged at the project root.

```
templates/
├── crates/web/src/scalar.rs           shared: scalar(), API_DOCS_CSP, self-hosted page + tests
├── actix/
│   ├── crates/web/src/openapi.rs      ApiDoc, SecurityAddon, collect_routes, openapi_spec
│   ├── crates/web/src/handlers/example.rs  one annotated handler + DTO (route macro)
│   ├── crates/web/tests/openapi_contract.rs  body + status lint
│   └── server/src/main.rs             dump-spec subcommand, server wiring, /scalar CSP
├── axum/
│   ├── crates/web/Cargo.toml          `server` feature, off for swaggergen
│   ├── crates/web/src/{lib,openapi}.rs ApiDoc, SecurityAddon, api_router, app_router, openapi_spec
│   ├── crates/web/src/handlers/example.rs  one annotated handler + DTO (no route macro)
│   ├── crates/web/tests/openapi_contract.rs  body gate with NO_BODY + self-test
│   ├── server/src/main.rs             server wiring, /scalar CSP
│   └── apps/swaggergen/               prints the spec
├── ui/openapi-ts.config.ts            axios client into src/client
├── ui/src/clientInit.ts               baseURL + bearer token
├── ui/build/copy-scalar.mjs           self-host the Scalar bundle
├── .tasks/api.yaml                    dump-spec, ui:generate, generate, openapi:lint
├── .spectral.yaml                     OAS + OWASP ruleset, every exception justified
└── .github/workflows/openapi-lint.yaml
```

Replace every `my_crate` / `my-crate` / `my-server` placeholder; on axum, switch `dump-spec` in
`.tasks/api.yaml` to swaggergen. Raise `MIN_ROUTES` in the contract
test as the API grows. Related skills: `weebo-testing` (where the contract test runs, coverage),
`weebo-taskfile-mise` (Taskfile includes, mise), `weebo-security` (`task audit`, which
`openapi:lint` sits beside), `weebo-containerfile` (shipping `ui/dist` with the server).
