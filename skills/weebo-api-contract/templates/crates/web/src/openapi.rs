// source: batlehub@20879e5 (crates/web/src/lib.rs, crates/web/src/middleware/security_headers.rs), trimmed.
//
// The whole backend half of the contract: the `ApiDoc` root, route collection
// through utoipa-actix-web, `openapi_spec()` for dump-spec and the contract test,
// and the self-hosted Scalar page. `pub mod openapi; pub use openapi::*;` in lib.rs.

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;
use utoipa_actix_web::{service_config::ServiceConfig as UtoipaServiceConfig, AppExt};
use utoipa_scalar::{Scalar, Servable as _};

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

/// Declares the `bearer_token` scheme that handlers name in `security(...)`.
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

/// Where the self-hosted Scalar bundle lives, relative to this origin.
/// Written there by `ui/build/copy-scalar.mjs` from the `@scalar/api-reference`
/// devDependency.
///
/// Why not the CDN: `utoipa-scalar`'s stock template loads
/// `https://cdn.jsdelivr.net/npm/@scalar/api-reference` with no version and no
/// integrity, on the origin that holds the console's tokens. It is also a blank
/// page with no egress, and invisible to `pnpm audit` and the SBOM.
pub const SCALAR_BUNDLE_PATH: &str = "assets/scalar/standalone.js";

pub fn scalar_bundle_present(static_dir: Option<&std::path::Path>) -> bool {
    static_dir.is_some_and(|dir| dir.join(SCALAR_BUNDLE_PATH).is_file())
}

/// `$spec` is substituted by `Scalar::to_html`, inside a
/// `type="application/json"` block, so it is data and the CSP needs no
/// `'unsafe-inline'` for scripts.
///
/// `withDefaultFonts: false` stops fonts.scalar.com; `proxyUrl: ""` stops
/// "Test Request" from routing the operator's request (and any pasted token)
/// through proxy.scalar.com. The third origin, api.scalar.com, is hardcoded in
/// the bundle: only `connect-src 'self'` in [`API_DOCS_CSP`] stops it.
fn scalar_html() -> String {
    format!(
        r#"<!doctype html>
<html>
<head>
    <title>my-crate API</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
</head>
<body>
<script id="api-reference"
        type="application/json"
        data-configuration='{{"withDefaultFonts":false,"proxyUrl":""}}'>
    $spec
</script>
<script src="/{SCALAR_BUNDLE_PATH}"></script>
</body>
</html>
"#
    )
}

/// Served when the bundle is not on disk. No CDN fallback: it would silently
/// reinstate the third-party script exactly where it is least wanted. The spec
/// is still embedded, so `curl /scalar` yields it.
fn scalar_unavailable_html() -> String {
    format!(
        r#"<!doctype html>
<html>
<head>
    <title>my-crate API</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
</head>
<body>
<h1>API reference unavailable</h1>
<p>Build the UI (<code>pnpm --dir ui run build</code>) and serve <code>ui/dist</code>:
<code>{SCALAR_BUNDLE_PATH}</code> is missing.</p>
<script id="api-reference" type="application/json">
    $spec
</script>
</body>
</html>
"#
    )
}

/// The `/scalar` service. Register it on the plain `App` after `split_for_parts()`.
pub fn scalar(
    openapi: utoipa::openapi::OpenApi,
    static_dir: Option<&std::path::Path>,
) -> Scalar<utoipa::openapi::OpenApi> {
    let html = if scalar_bundle_present(static_dir) {
        scalar_html()
    } else {
        scalar_unavailable_html()
    };
    Scalar::with_url("/scalar", openapi).custom_html(html)
}

/// The `Content-Security-Policy` to send on `/scalar`. `connect-src 'self'` is
/// deliberate (see [`scalar_html`]), not the `*` an API explorer seems to want.
pub const API_DOCS_CSP: &str = "default-src 'none'; \
     script-src 'self'; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: blob:; \
     font-src 'self' data:; \
     connect-src 'self'; \
     worker-src 'self' blob:; \
     base-uri 'none'; \
     form-action 'none'; \
     frame-ancestors 'none'";

#[cfg(test)]
mod scalar_tests {
    use super::*;

    #[test]
    fn the_reference_loads_nothing_from_a_third_party() {
        for html in [scalar_html(), scalar_unavailable_html()] {
            assert!(!html.contains("//"), "no external URL may appear: {html}");
            assert!(!html.contains("scalar.com"));
        }
    }

    /// Losing `$spec` would serve an empty reference that still looks fine.
    #[test]
    fn both_templates_keep_the_spec_placeholder() {
        assert!(scalar_html().contains("$spec"));
        assert!(scalar_unavailable_html().contains("$spec"));
    }

    #[test]
    fn without_a_bundle_the_degraded_page_is_served() {
        let rendered = scalar(openapi_spec(), None).to_html();
        assert!(rendered.contains("API reference unavailable"));
        assert!(!rendered.contains("<script src="));
    }
}
