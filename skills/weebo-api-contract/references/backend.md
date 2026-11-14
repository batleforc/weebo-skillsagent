# Backend: utoipa, route collection, Scalar, dump-spec

Templates: `templates/actix/` (batlehub), `templates/axum/` (ProxyAuthK8S), and the shared
`templates/crates/web/src/scalar.rs`. §1–§2 and §4 describe actix then axum; §3 is the same on both.
Off Rust, the rules (named bodies, one route list, self-hosted Scalar, a spec export) carry over to
the framework's own generator; see SKILL.md §4.

## 1. Annotating a handler

**actix.** Each handler carries both macros: `#[utoipa::path(...)]` for the document and the actix route macro
(`#[get("/api/v1/...")]`) for the router, with the same path in both.

```rust
/// Fetch one item by id.                 // → summary (Spectral requires it)
#[utoipa::path(
    get,
    path = "/api/v1/items/{id}",
    tag = "items",                        // declared in ApiDoc `tags(...)`
    params(("id" = String, Path, description = "Item id")),
    responses(
        (status = 200, description = "The item", body = ItemResponse),
        (status = 404, description = "No such item"),
    ),
    security(("bearer_token" = [])),
)]
#[get("/api/v1/items/{id}")]
pub async fn get_item(...)
```

- DTOs derive `ToSchema` beside `Serialize`/`Deserialize`, in the handler's module. Field `///`
  comments become schema descriptions and end up as JSDoc in the generated types.
- A handler that returns an ad-hoc `json!` gets a named DTO instead: an unnamed body cannot be
  documented, so the client types it `unknown`.
- A request body is `request_body = T`. A `204` declares no body.
- utoipa 6 with `actix_extras` (plus `chrono`, `uuid` when DTOs use them).

**axum.** Only `#[utoipa::path(...)]`: `routes!(get_item)` reads method and path from it, so there is
no second copy of the path to keep in sync. The path is relative to the `nest` prefix the handler is
mounted under (`path = "/items/{id}"` under `.nest("/api/v1", …)` documents `/api/v1/items/{id}`).
utoipa 6 with `axum_extras`, which also documents `Path`/`Query` extractors.

## 2. ApiDoc and route collection

`ApiDoc` (`#[derive(OpenApi)]`) holds only the document-wide parts: `info` (with `contact`, which
`spectral:oas` requires), `tags`, and `modifiers(&SecurityAddon)`, which declares the
`bearer_token` HTTP bearer scheme that handlers name in `security(...)`. It lists no paths.

Paths come from utoipa-actix-web: `collect_routes(cfg: &mut utoipa_actix_web::service_config::ServiceConfig)`
calls `cfg.service(handler)` for every handler, and each registration adds the route and its path
item. Two callers use it:

- **the server**: `App::new().into_utoipa_app().openapi(ApiDoc::openapi()).configure(...).split_for_parts()`
  returns the plain actix `App` and the finished `OpenApi`;
- **`openapi_spec()`**: the same chain on a throwaway `App`, keeping only the spec. `dump-spec` and
  the contract test call it. It needs no config, database or network.

In batlehub `configure_app(...)` takes the services and returns a closure that registers app data
then calls `collect_routes`; the template keeps only `collect_routes`. Register static files
(`actix_files::Files`, the SPA fallback) on the plain `App` **after** `split_for_parts()`: `Files`
is not an `OpenApiFactory` and would disturb path collection.

Order matters for actix matching, not for the spec: register the more specific path first
(`subscriptions/{id}/test` before `subscriptions/{id}`).

**axum.** One `OpenApiRouter<SharedState>` per prefix (`api_router()`, ProxyAuthK8S also has
`management_router()` and `cluster_router()`), each `.routes(routes!(handler))`. `app_router()` nests
them into `OpenApiRouter::with_openapi(ApiDoc::openapi())` and returns `.split_for_parts()`: the
plain `axum::Router` and the finished spec. The server and `openapi_spec()` (`app_router().1`) both
call it.

- A layer for some routes only (ProxyAuthK8S: `DefaultBodyLimit` on its form endpoints) goes on a
  sub-router merged into the prefix router; the spec is unaffected.
- A route that `routes!` cannot express (ProxyAuthK8S's proxy catch-all, routed as `{*path}` with
  `.route(...)`) is served but not collected: list its handlers in `ApiDoc`'s `paths(...)`, written
  `{path}` there. That is the only case where `ApiDoc` names paths.
- A route kept out of the spec on purpose (ProxyAuthK8S's issuer endpoints, read by apiservers only)
  is merged into the plain `Router` after `split_for_parts()`, with a comment saying why.
- `ApiDoc::openapi()` is mutated before use to set `info.version` from `CARGO_PKG_VERSION`.
- Static files (`tower_http::services::ServeDir`, the SPA fallback) go on the plain `Router` after
  `split_for_parts()`, like actix.

## 3. Scalar, self-hosted

`scalar(openapi, static_dir)` returns `Scalar::with_url("/scalar", openapi).custom_html(html)`
(`Servable` must be in scope for `with_url`). The custom HTML replaces utoipa-scalar's stock template,
which loads `https://cdn.jsdelivr.net/npm/@scalar/api-reference` with no version and no integrity,
on the origin that holds the console's bearer and refresh tokens. Self-hosting also fixes the blank
page on a server with no egress, the IP/referrer leak, and makes the bundle's ~190 transitive
dependencies visible to `pnpm audit` and the SBOM.

- `ui/build/copy-scalar.mjs` copies `@scalar/api-reference/dist/browser/standalone.js` (the IIFE, not
  the code-split ESM build) to `ui/dist/assets/scalar/standalone.js`. It runs at the end of
  `pnpm run build`.
- `SCALAR_BUNDLE_PATH` in Rust must match that target. The bundle is served by the console's static
  mount; `assets/` is build-owned, so a missing file is a 404, not the SPA's `index.html`.
- `data-configuration='{"withDefaultFonts":false,"proxyUrl":""}'` turns off fonts.scalar.com and
  proxy.scalar.com ("Test Request" would otherwise send the request and any pasted token through it).
- api.scalar.com is hardcoded in the bundle. Only `connect-src 'self'` in `API_DOCS_CSP` stops it,
  which is why that directive is not `*`.
- No bundle on disk (no `static_dir`, or a server built without the UI): a degraded page that says
  what is missing. No CDN fallback; it would reinstate the script silently, exactly on air-gapped
  installs. Both pages embed `$spec` in a `type="application/json"` block, so `curl /scalar` still
  yields the spec and the CSP needs no `'unsafe-inline'` for scripts.
- `API_DOCS_CSP` is sent only on `/scalar`: the console and the API docs need different policies, so
  it cannot be global. Batlehub does it in a prefix-keyed middleware; the templates use
  `actix_web::middleware::from_fn` and `axum::middleware::from_fn`.
- Mounting: actix `.service(scalar(..))` on the plain `App`; axum `.merge(scalar(..))` on the
  `Router` (utoipa-scalar's `axum` feature turns `Scalar` into a `Router`).
- ProxyAuthK8S diverges here: it serves Scalar at `/api/docs` from jsDelivr, pinned with SRI. The
  axum template follows the house rule instead; a pin with SRI still leaves the egress, the leak and
  the bundle outside the audit.
- Unit tests in `openapi.rs` pin all of this: no `//` or `scalar.com` in either page, `$spec` kept,
  degraded page served without a bundle.

## 4. Exporting the spec

**actix: `dump-spec`**, a clap subcommand of the server binary:

```rust
Some(Command::DumpSpec) => {
    let spec = my_crate::openapi_spec();
    println!("{}", spec.to_pretty_json().expect("serialize openapi spec"));
}
```

`task dump-spec` runs `cargo run -p my-server -- dump-spec > ui/openapi.json`. Batlehub passes
`--config config.example.toml` because its CLI parses config before dispatching; a server whose
subcommands run before config loading does not need it. Output is OpenAPI 3.1.

**axum: `swaggergen`**, a separate binary (`apps/swaggergen`) that prints `openapi_spec()`.
`task dump-spec` runs `cargo run -p swaggergen > ui/openapi.json`. It depends on the API crate with
`default-features = false`, and the crate puts the server stack (utoipa-scalar there, OTLP, CORS,
TLS in ProxyAuthK8S) behind a default `server` feature, so the export builds only handlers and DTOs.

Pick by weight: a subcommand is one binary less; a separate binary pays off once the server stack is
slow to build or needs config to start. The task name stays `dump-spec` either way.
