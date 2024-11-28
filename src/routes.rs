use std::sync::Arc;

use axum::{extract::State, routing::get, Router};

use crate::token_service::TokenService;


pub fn get_router(app_state: Arc<AppState>) -> Router {
    Router::new()
    .route("/", get(root))
    .route("/tokens/:address", get(get_tokens))
    .with_state(app_state)
}

async fn root() -> &'static str {
    "Hello, World!"
}

async fn get_tokens(
    State(state): State<Arc<AppState>>,
    address: axum::extract::Path<String>,
) -> axum::response::Json<serde_json::Value> {
    let wallet_address = address.to_string();
    let tokens = state.token_service.fetch_tokens(&wallet_address).await.unwrap_or_default();
    axum::response::Json(serde_json::json!({ "tokens": tokens }))
}

#[derive(Clone)]
pub struct AppState {
    pub token_service: Arc<TokenService>,
}