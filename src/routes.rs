use axum::{routing::get, Router};

use crate::tokens_fetcher::fetch_tokens;

pub fn get_router() -> Router {
    Router::new()
    .route("/", get(root))
    .route("/tokens/:address", get(get_tokens))
}

async fn root() -> &'static str {
    "Hello, World!"
}

async fn get_tokens(
    address: axum::extract::Path<String>,
) -> axum::response::Json<serde_json::Value> {
    let wallet_address = address.to_string();
    let tokens = fetch_tokens(&wallet_address).await.unwrap_or_default();
    axum::response::Json(serde_json::json!({ "tokens": tokens }))
}