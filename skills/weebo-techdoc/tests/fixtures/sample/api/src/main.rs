use axum::{extract::State, http::{HeaderMap, StatusCode}, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Serialize, sqlx::FromRow)]
struct Item {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
struct NewItem {
    name: String,
}

async fn list_items(State(db): State<PgPool>) -> Json<Vec<Item>> {
    let items = sqlx::query_as("SELECT id, name FROM items")
        .fetch_all(&db)
        .await
        .unwrap();
    Json(items)
}

async fn create_item(
    State(db): State<PgPool>,
    headers: HeaderMap,
    Json(new): Json<NewItem>,
) -> Result<Json<Item>, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !auth_core::verify_token(token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let item = sqlx::query_as("INSERT INTO items (name) VALUES ($1) RETURNING id, name")
        .bind(new.name)
        .fetch_one(&db)
        .await
        .unwrap();
    Ok(Json(item))
}

#[tokio::main]
async fn main() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = PgPool::connect(&url).await.unwrap();
    let app = Router::new()
        .route("/items", get(list_items).post(create_item))
        .with_state(db);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
