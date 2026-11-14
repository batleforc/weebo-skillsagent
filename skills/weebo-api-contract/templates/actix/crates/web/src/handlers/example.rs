// source: batlehub@20879e5 (crates/web/src/handlers/auth/tokens.rs, handlers/flags.rs), trimmed.
//
// One annotated handler. The rules it shows:
// - the `///` first line becomes the operation `summary` (Spectral requires it);
// - every 200/201 names a `body = T` (the contract test requires it);
// - the declared 2xx is the one the body constructs (the source lint requires it);
// - the fn name becomes the operationId, so it is the generated SDK function name
//   and must be unique across the crate.

use actix_web::{get, web, HttpResponse, Responder};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct ItemResponse {
    pub id: String,
    /// Field doc comments become schema descriptions.
    pub name: String,
}

/// Fetch one item by id.
#[utoipa::path(
    get,
    path = "/api/v1/items/{id}",
    tag = "items",
    params(("id" = String, Path, description = "Item id")),
    responses(
        (status = 200, description = "The item", body = ItemResponse),
        (status = 404, description = "No such item"),
    ),
    security(("bearer_token" = [])),
)]
#[get("/api/v1/items/{id}")]
pub async fn get_item(path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    HttpResponse::Ok().json(ItemResponse {
        name: format!("item {id}"),
        id,
    })
}
