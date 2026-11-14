// source: batlehub@20879e5 (crates/web/src/lib.rs, crates/web/src/middleware/security_headers.rs), trimmed.
//
// The actix half of the contract: the `ApiDoc` root, route collection through
// utoipa-actix-web, and `openapi_spec()` for dump-spec and the contract test.
// The Scalar page is framework-agnostic: templates/crates/web/src/scalar.rs.
// `pub mod openapi; pub mod scalar; pub use openapi::*; pub use scalar::*;` in lib.rs.

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;
use utoipa_actix_web::{service_config::ServiceConfig as UtoipaServiceConfig, AppExt};

#[derive(OpenApi)]
#[openapi(
    info(
        description = "my-crate API",
        // spectral:oas `info-contact` fails without it.
        contact(name = "my-crate", url = "https://github.com/my-org/my-crate"),
    ),
    tags((name = "items", description = "Items")),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

/// Declares the bearer scheme that handlers name in `security(...)`. The name is
/// the project's choice (batlehub: `bearer_token`); handlers must use the same.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
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

/// Every documented route, registered once. Both the running server and
/// `openapi_spec()` go through this, so the served API and the dumped spec
/// cannot list different routes.
pub fn collect_routes(cfg: &mut UtoipaServiceConfig) {
    use crate::handlers::example::get_item;
    cfg.service(get_item);
}

/// Return the raw OpenAPI spec (auto-collected from route registrations).
pub fn openapi_spec() -> utoipa::openapi::OpenApi {
    let (_, openapi) = actix_web::App::new()
        .into_utoipa_app()
        .openapi(ApiDoc::openapi())
        .configure(collect_routes)
        .split_for_parts();
    openapi
}
