use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    token_service::TokenService, trade_service::TradeService, trade_websocket::handle_socket,
};

pub fn get_router(app_state: Arc<AppState>, sessions: Arc<SharedSessions>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/tokens/:address", get(get_tokens))
        .route("/trade", post(create_trade_session))
        .route("/ws", get(websocket_handler))
        .with_state(app_state)
        .layer(Extension(sessions))
}

async fn root() -> &'static str {
    "Hello, World!"
}

pub type SessionId = Uuid;
pub type ConnectionId = Uuid;

pub struct SharedSessions {
    internal: RwLock<HashMap<SessionId, TradeSession>>,
}
impl SharedSessions {
    pub fn new() -> Self {
        SharedSessions {
            internal: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_client(
        &self,
        session_id: SessionId,
        connection_id: ConnectionId,
        tx: mpsc::Sender<String>,
    ) {
        let mut sessions = self.internal.write().unwrap();
        sessions
            .entry(session_id)
            .or_default()
            .ws_clients
            .insert(connection_id, tx);
    }

    pub fn remove_client(&self, session_id: &SessionId, connection_id: &ConnectionId) {
        let mut sessions = self.internal.write().unwrap();
        if let Some(trade_session) = sessions.get_mut(&session_id) {
            trade_session.ws_clients.remove(&connection_id);
        }
    }
    pub fn broadcast(&self, session_id: &SessionId, msg: &str) {
        let sessions = self.internal.read().unwrap();
        if let Some(clients) = sessions.get(session_id) {
            for tx in clients.ws_clients.values() {
                let _ = tx.try_send(msg.to_owned());
            }
        }
    }
}
#[derive(Default)]
pub struct TradeSession {
    pub state: TradeState,
    pub ws_clients: HashMap<ConnectionId, mpsc::Sender<String>>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TradeState {
    items: Vec<String>,
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<ConnectParams>,
    Extension(sessions): Extension<Arc<SharedSessions>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, params.session_id, sessions))
}

async fn create_trade_session(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTradeSession>,
) -> axum::http::Response<axum::body::Body> {
    match state
        .trade_service
        .create_trade_session(&payload.initiator_address)
    {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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

#[derive(Deserialize)]
struct ConnectParams {
    session_id: Uuid,
}

#[derive(Clone)]
pub struct AppState {
    pub token_service: Arc<TokenService>,
    pub trade_service: Arc<TradeService>,
}
