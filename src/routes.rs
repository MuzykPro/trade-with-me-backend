use std::sync::Arc;

use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{token_service::TokenService, trade_service::TradeService};

pub fn get_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/tokens/:address", get(get_tokens))
        .route("/trade", post(create_trade_session))
        .with_state(app_state)
}

async fn root() -> &'static str {
    "Hello, World!"
}
async fn create_trade_session(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTradeSession>,
) -> axum::http::Response<axum::body::Body> {
    match state
        .trade_service
        .create_trade_session(&payload.initiator_address) {
            Ok(()) => StatusCode::CREATED.into_response(),
            Err(e) => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string(),
                ).into_response()
            }
        }
    
}

async fn get_tokens(
    State(state): State<Arc<AppState>>,
    address: axum::extract::Path<String>,
) -> axum::response::Json<serde_json::Value> {
    let wallet_address = address.to_string();
    let tokens = state
        .token_service
        .fetch_tokens(&wallet_address)
        .await
        .unwrap_or_default();
    axum::response::Json(serde_json::json!({ "tokens": tokens }))
}
#[derive(Deserialize)]
struct CreateTradeSession {
    initiator_address: String,
}

#[derive(Clone)]
pub struct AppState {
    pub token_service: Arc<TokenService>,
    pub trade_service: Arc<TradeService>,
}
