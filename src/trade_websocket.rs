use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use log::info;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::trade_session::{SessionId, SharedSessions};


pub async fn handle_socket(
    socket: WebSocket,
    session_id: SessionId,
    sessions: Arc<SharedSessions>,
) {
    let connection_id = Uuid::new_v4();

    let (tx, mut rx) = mpsc::channel(32);

    sessions.add_client(session_id, connection_id, tx);

    let (mut ws_sink, mut ws_stream) = socket.split();

    let write_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let msg_json_result = serde_json::to_string(&msg);
            if let Ok(msg_json) = msg_json_result {
                if ws_sink.send(Message::Text(msg_json)).await.is_err() {
                    // If send fails, client disconnected
                    break;
                }
            }
        }
    });

    let read_handle = tokio::spawn({
        let sessions = Arc::clone(&sessions);
        async move {
            while let Some(Ok(msg)) = ws_stream.next().await {
                match msg {
                    Message::Text(text) => {
                        info!("Received from client {}: {}", connection_id, text);
                        if let Ok(msg) = serde_json::from_str::<WebsocketMessage>(&text) {
                            match msg {
                                WebsocketMessage::OfferTokens{user_address, token_mint, amount} => {
                                    //TODO handle errors
                                    sessions.add_tokens_offer(&session_id, user_address, token_mint, amount);
                                    sessions.broadcast_current_state(&session_id);
                                },
                                WebsocketMessage::WithdrawTokens{user_address, token_mint, amount} => {
                                    //TODO handle errors
                                    sessions.withdraw_tokens(&session_id, user_address, token_mint, amount);
                                    sessions.broadcast_current_state(&session_id);
                                },
                                _ => {}
                            }
                        }
                    }
                    Message::Close(_frame) => {
                        info!(
                            "Client {} disconnected from session {}",
                            connection_id, session_id
                        );
                        break;
                    }
                    _ => {}
                }
            }
        }
    });

    let _ = tokio::join!(write_handle, read_handle);

    sessions.remove_client(&session_id, &connection_id);
    info!(
        "Removed client {} from session {}",
        connection_id, session_id
    );
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebsocketMessage {
    OfferTokens {
        #[serde(rename = "userAddress")] 
        user_address: String,
        #[serde(rename = "tokenMint")] 
        token_mint: String,
        amount: u64
    },
    WithdrawTokens {
        #[serde(rename = "userAddress")] 
        user_address: String,
        #[serde(rename = "tokenMint")] 
        token_mint: String,
        amount: u64
    },
    TradeStateUpdate {
        offers: Arc<HashMap<String, HashMap<String, u64>>>
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenOffer {
        pub mint: String,
        pub amount: u64
}