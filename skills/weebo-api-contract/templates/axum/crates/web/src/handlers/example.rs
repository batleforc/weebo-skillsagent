// source: ProxyAuthK8S@8fe220b (libs/server/api/src/base.rs, visible_clusters), trimmed.
//
// One annotated handler. The rules it shows:
// - the `///` first line becomes the operation `summary` (Spectral requires it);
// - every 200/201 names a `body = T` (the contract test requires it);
// - no route macro: `routes!(get_item)` reads method and path from the annotation,
//   and the path is relative to the `nest` prefix (`/api/v1`);
// - the fn name becomes the operationId, so it is the generated SDK function name
//   and must be unique across the crate.

use axum::{Json, extract::Path};
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
    path = "/items/{id}",
    tag = "items",
    params(("id" = String, Path, description = "Item id")),
    responses(
        (status = 200, description = "The item", body = ItemResponse),
        (status = 404, description = "No such item"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn get_item(Path(id): Path<String>) -> Json<ItemResponse> {
    Json(ItemResponse {
        name: format!("item {id}"),
        id,
    })
}
