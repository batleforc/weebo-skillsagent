// source: batlehub@20879e5 (server/src/main.rs, server/src/server_factory.rs, crates/web/src/middleware), trimmed.
//
// The binary: `my-server dump-spec` prints the spec (what `task dump-spec` and CI
// run), anything else serves the API with `/scalar` mounted.

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{HeaderValue, CONTENT_SECURITY_POLICY};
use actix_web::middleware::{from_fn, Next};
use actix_web::{App, HttpServer};
use clap::{Parser, Subcommand};
use utoipa::OpenApi as _;
use utoipa_actix_web::AppExt;

#[derive(Parser)]
struct Cli {
    /// Built UI (`ui/dist`); `/scalar` loads its bundle from here.
    #[arg(long)]
    static_dir: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the OpenAPI spec to stdout and exit (for frontend code generation).
    DumpSpec,
}

/// `/scalar` gets its own CSP; it cannot be global because the console and the
/// API docs each need a different policy.
async fn docs_csp(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let is_docs = req.path().starts_with("/scalar");
    let mut res = next.call(req).await?;
    if is_docs {
        res.headers_mut().insert(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(my_crate::API_DOCS_CSP),
        );
    }
    Ok(res)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    if let Some(Command::DumpSpec) = cli.command {
        let spec = my_crate::openapi_spec();
        println!("{}", spec.to_pretty_json().expect("serialize openapi spec"));
        return Ok(());
    }

    let static_dir = cli.static_dir;
    HttpServer::new(move || {
        let (app, openapi) = App::new()
            .into_utoipa_app()
            .openapi(my_crate::ApiDoc::openapi())
            .configure(my_crate::collect_routes)
            .split_for_parts();
        // Static files (actix_files::Files) go on this plain App, after
        // split_for_parts: they are not an OpenApiFactory and would disturb
        // path collection.
        app.wrap(from_fn(docs_csp))
            .service(my_crate::scalar(openapi, static_dir.as_deref()))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
