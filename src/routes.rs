use std::sync::Arc;

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::{
    chain_context::{ChainContext}, token_service::TokenService, trade_service::TradeService, trade_session::SharedSessions, trade_websocket::handle_socket
};

pub fn get_router<T: ChainContext + Sync + Send + 'static>(app_state: Arc<AppState>, sessions: Arc<SharedSessions<T>>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/tokens", get(get_tokens))
        .route("/tokens/metadata", get(get_token_metadata))
        .route("/trading_session", post(create_trade_session))
        .route("/ws/trading_session/:session_id", get(websocket_handler::<T>))
        .with_state(app_state)
        .layer(Extension(sessions))
        .layer(CorsLayer::permissive())
}

async fn root() -> &'static str {
    "Hello, World!"
}

async fn get_token_metadata(
    State(state): State<Arc<AppState>>,
    query_params: axum::extract::Query<GetTokenMetadataQuery>,
) -> axum::http::Response<axum::body::Body> {
    if let Some(metadata) = state
        .token_service
        .get_token_metadata(&query_params.mint_address)
        .await
    {
        (StatusCode::CREATED, Json(metadata)).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!(
                "Metadata for token {} not found",
                &query_params.mint_address
            ),
        )
            .into_response()
    }
}

async fn websocket_handler<T: ChainContext + Sync + Send + 'static>(
    ws: WebSocketUpgrade,
    Path(params): Path<SessionPathParam>,
    Extension(sessions): Extension<Arc<SharedSessions<T>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, params.session_id, sessions))
}

#[derive(Deserialize)]
struct CreateTradeSession {
    #[serde(rename = "initiatorAddress")]
    initiator_address: String,
}

async fn create_trade_session(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTradeSession>,
) -> axum::http::Response<axum::body::Body> {
    match state
        .trade_service
        .create_trade_session(&payload.initiator_address)
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(CreateTradeSessionResponse {
                uuid: id.to_string(),
            }),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Serialize)]
pub struct CreateTradeSessionResponse {
    uuid: String,
}

#[derive(Deserialize)]
pub struct GetTokensQuery {
    address: String,
}

#[derive(Deserialize)]
pub struct GetTokenMetadataQuery {
    mint_address: String,
}

async fn get_tokens(
    State(state): State<Arc<AppState>>,
    query_params: axum::extract::Query<GetTokensQuery>,
) -> axum::response::Json<serde_json::Value> {
    let wallet_address = &query_params.address;
    let tokens = state
        .token_service
        .fetch_tokens(wallet_address)
        .await
        .unwrap_or_default();
    axum::response::Json(serde_json::json!({ "tokens": tokens }))
}

#[derive(Deserialize)]
struct SessionPathParam {
    session_id: Uuid,
}

#[derive(Clone)]
pub struct AppState {
    pub token_service: Arc<TokenService>,
    pub trade_service: Arc<TradeService>,
}
