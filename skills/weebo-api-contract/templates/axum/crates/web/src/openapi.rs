// source: ProxyAuthK8S@8fe220b (libs/server/api/src/lib.rs, libs/server/api/src/api_doc.rs), trimmed.
//
// The axum half of the contract: the `ApiDoc` root, one `OpenApiRouter` per
// prefix, `app_router()` that both the server and `openapi_spec()` call.

use std::sync::Arc;

use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(OpenApi)]
#[openapi(
    info(
        description = "my-crate API",
        // spectral:oas `info-contact` fails without it.
        contact(name = "my-crate", url = "https://github.com/my-org/my-crate"),
    ),
    tags((name = "items", description = "Items")),
    // Only routes registered outside `routes!` belong here (ProxyAuthK8S: its
    // proxy catch-all, routed as `{*path}` and documented as `{path}`). Any
    // other route added with plain `.route(...)` is served but undocumented.
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

/// Declares the bearer scheme that handlers name in `security(...)`. The name is
/// the project's choice (ProxyAuthK8S: `bearer_auth`); handlers must use the same.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // A modifier must not panic when no component is registered yet.
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_token",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("token")
                    .build(),
            ),
        );
    }
}

/// What every handler shares. Replace with the project's state.
#[derive(Default)]
pub struct AppState {}

pub type SharedState = Arc<AppState>;

/// `/api/v1`. A layer that must cover only some routes (a body limit on a
/// form endpoint) goes on a sub-router merged in here; the spec is unaffected.
pub fn api_router() -> OpenApiRouter<SharedState> {
    OpenApiRouter::new().routes(routes!(crate::handlers::example::get_item))
}

/// Every route, and the OpenAPI document they describe. The running server
/// and `openapi_spec()` both go through this, so they cannot list different
/// routes. Paths in `#[utoipa::path]` are relative to the `nest` prefix.
pub fn app_router() -> (axum::Router<SharedState>, utoipa::openapi::OpenApi) {
    let mut doc = ApiDoc::openapi();
    doc.info.version = env!("CARGO_PKG_VERSION").to_string();
    OpenApiRouter::with_openapi(doc)
        .nest("/api/v1", api_router())
        .split_for_parts()
}

/// The spec alone, for swaggergen and the contract test. Needs no config,
/// database or network.
pub fn openapi_spec() -> utoipa::openapi::OpenApi {
    app_router().1
}
