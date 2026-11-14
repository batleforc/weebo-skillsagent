// source: batlehub@20879e5 (crates/web/src/lib.rs, crates/web/src/middleware/security_headers.rs), trimmed.
//
// The self-hosted Scalar page and its CSP. Framework-agnostic: utoipa-scalar
// serves it on actix (`.service(scalar(..))`) and on axum (`.merge(scalar(..))`)
// once its `actix-web` or `axum` feature is on.

use utoipa_scalar::{Scalar, Servable as _};

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

/// The `/scalar` service. Register it after `split_for_parts()` (actix: on the
/// plain `App`; axum: merged into the `Router`).
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
    use crate::openapi_spec;

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
