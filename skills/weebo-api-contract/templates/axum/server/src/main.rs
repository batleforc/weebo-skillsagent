// source: ProxyAuthK8S@8fe220b (libs/server/api/src/server.rs, apps/server/src/main.rs), trimmed.
//
// The binary: serves the API with `/scalar` mounted. The spec is dumped by
// apps/swaggergen, not by a subcommand here.
//
// Divergence: ProxyAuthK8S loads Scalar from a pinned CDN URL with SRI. The
// house rule is the self-hosted bundle (references/backend.md §3), so this
// mounts `my_crate::scalar` with its own CSP instead.

use std::sync::Arc;

use axum::extract::Request;
use axum::http::{HeaderValue, header::CONTENT_SECURITY_POLICY};
use axum::middleware::{self, Next};
use axum::response::Response;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    /// Built UI (`ui/dist`); `/scalar` loads its bundle from here.
    #[arg(long)]
    static_dir: Option<std::path::PathBuf>,
}

/// `/scalar` gets its own CSP; it cannot be global because the console and the
/// API docs each need a different policy.
async fn docs_csp(req: Request, next: Next) -> Response {
    let is_docs = req.uri().path().starts_with("/scalar");
    let mut res = next.run(req).await;
    if is_docs {
        res.headers_mut().insert(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(my_crate::API_DOCS_CSP),
        );
    }
    res
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let (router, openapi) = my_crate::app_router();
    // Static files (tower_http::services::ServeDir over `static_dir`, the SPA
    // fallback) go on this plain Router, after split_for_parts: they are not in
    // the spec. The Scalar bundle is served from there.
    let app = router
        .merge(my_crate::scalar(openapi, cli.static_dir.as_deref()))
        .layer(middleware::from_fn(docs_csp))
        .with_state(Arc::new(my_crate::AppState::default()));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", 8080)).await?;
    axum::serve(listener, app).await
}
