use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use log::{debug, info};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{chain_context::ChainContext, trade_session::{SessionId, SharedSessions}};

pub async fn handle_socket<T: ChainContext + Sync + Send + 'static>(
    socket: WebSocket,
    session_id: SessionId,
    sessions: Arc<SharedSessions<T>>,
) {
    let connection_id = Uuid::new_v4();

    let (tx, mut rx) = mpsc::channel(32);

    sessions.add_client(session_id, connection_id, tx);
    sessions.broadcast_current_state(&session_id);

    let (mut ws_sink, mut ws_stream) = socket.split();

    let write_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let msg_json_result = serde_json::to_string(&msg);
            if let Ok(msg_json) = msg_json_result {
                debug!("Sending ws message {:#?}", &msg_json);
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
                                WebsocketMessage::OfferTokens {
                                    user_address,
                                    token_mint,
                                    amount,
                                } => {
                                    //TODO handle errors
                                    let _ = sessions.add_tokens_offer(
                                        &session_id,
                                        &user_address,
                                        token_mint,
                                        amount,
                                    );
                                    sessions.broadcast_current_state(&session_id);
                                }
                                WebsocketMessage::WithdrawTokens {
                                    user_address,
                                    token_mint,
                                    amount,
                                } => {
                                    //TODO handle errors
                                    let _ = sessions.withdraw_tokens(
                                        &session_id,
                                        &user_address,
                                        token_mint,
                                        amount,
                                    );
                                    sessions.broadcast_current_state(&session_id);
                                }
                                WebsocketMessage::AcceptTrade { user_address
                                 } => {
                                    //TODO handle errors
                                    let _ = sessions.accept_trade(&session_id, &user_address);
                                    sessions.broadcast_current_state(&session_id);
                                 }
                                 WebsocketMessage::GetTransactionToSign { user_address
                                 } => {
                                    //TODO handle errors
                                    // let _ = sessions.get_transaction_to_sign(&session_id, &user_address);
                                    sessions.broadcast_current_state(&session_id);
                                 }
                                 WebsocketMessage::SignedTransaction { user_address, signature
                                 } => {
                                    //TODO handle errors
                                    // let _ = sessions.sign_transaction(&session_id, &signature);
                                    sessions.broadcast_current_state(&session_id);
                                 }
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
        amount: Decimal,
    },
    WithdrawTokens {
        #[serde(rename = "userAddress")]
        user_address: String,
        #[serde(rename = "tokenMint")]
        token_mint: String,
        amount: Decimal,
    },
    AcceptTrade {
        #[serde(rename = "userAddress")]
        user_address: String,
    },
    GetTransactionToSign {
        #[serde(rename = "userAddress")]
        user_address: String,
    },
    SignedTransaction {
        #[serde(rename = "userAddress")]
        user_address: String,
        signature: String
    },
    TradeStateUpdate {
        offers: Arc<HashMap<String, HashMap<String, Decimal>>>,
        #[serde(rename = "userActed")]
        user_acted: Option<String>,
        status: String
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenOffer {
    pub mint: String,
    pub amount: Decimal,
}

#[cfg(test)]
mod tests {
    use crate::{chain_context::TestChainContext, token_amount_cache::TokenAmountCache, transaction_service::TransactionService};

    use super::*; // If your code is in the same module/crate. Otherwise, import appropriately.
    use axum::{
        extract::{Path, WebSocketUpgrade},
        routing::get,
        Router,
    };
    use futures::{SinkExt, StreamExt};
    use log::LevelFilter;
    use rust_decimal_macros::dec;
    use std::{future::IntoFuture, sync::Arc};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_two_clients_add_tokens_and_both_receive_update() -> anyhow::Result<()> {
        env_logger::Builder::new()
            .filter(None, LevelFilter::Debug) // Set log level
            .is_test(true) // Ensures output works correctly during tests
            .init();
        // 1. Create shared state
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));

        let alice_address = String::from("Alice");
        let token_mint = String::from("TokenA");
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts(
            alice_address.clone(),
            HashMap::from([(token_mint.clone(), dec!(200.0))]),
        );

        let shared_sessions = Arc::new(SharedSessions::new(token_amount_cache, transaction_service));

        // 2. Set up an Axum router with a WebSocket route
        let app = Router::new().route(
            "/ws/:session_id",
            get({
                let sessions = Arc::clone(&shared_sessions);
                move |ws: WebSocketUpgrade, Path(session_id): Path<Uuid>| async move {
                    ws.on_upgrade(move |socket| handle_socket(socket, session_id, sessions))
                }
            }),
        );

        // 3. Bind to an ephemeral port and spawn the server
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(axum::serve(listener, app).into_future());

        // 4. Create a random session_id
        let session_id = Uuid::new_v4();

        // 5. Connect two clients to the same session
        let url_1 = format!("ws://{}/ws/{}", addr, session_id);
        let url_2 = format!("ws://{}/ws/{}", addr, session_id);

        let (mut ws1, _resp1) = connect_async(url_1).await?;
        let (mut ws2, _resp2) = connect_async(url_2).await?;

        // 6. Client1 sends an OfferTokens message
        let offer_tokens = WebsocketMessage::OfferTokens {
            user_address: alice_address.clone(),
            token_mint: token_mint.clone(),
            amount: dec!(100.1337),
        };
        let offer_json = serde_json::to_string(&offer_tokens)?;
        info!("Offer json: {:#?}", &offer_json);
        ws1.send(Message::Text(offer_json.into())).await?;

        // 7. Both clients should eventually receive a TradeStateUpdate

        // We'll read up to 2 messages from each client and look for the `TradeStateUpdate` variant.
        let mut received_update_ws1 = false;
        let mut received_update_ws2 = false;

        // Because each client might receive some messages in different orders, we'll attempt to read a few times.

        for _ in 0..3 {
            if let Some(Ok(msg)) = ws1.next().await {
                if let Message::Text(payload) = msg {
                    if let Ok(parsed) = serde_json::from_str::<WebsocketMessage>(&payload) {
                        if let WebsocketMessage::TradeStateUpdate { offers, user_acted, status } = parsed {
                            if let Some(alice_map) = offers.get(&alice_address) {
                                received_update_ws1 = true;
                                // Check the data if needed:
                                // let maybe_alice = offers.get(&alice_address);
                                // assert!(maybe_alice.is_some(), "No 'Alice' user in update");
                                // let alice_map = alice.unwrap();
                                assert_eq!(alice_map.get(&token_mint), Some(&dec!(100.1337)));
                            }
                            
                        }
                    }
                }
            }
        }

        for _ in 0..2 {           
            if let Some(Ok(msg)) = ws2.next().await {
                if let Message::Text(payload) = msg {
                    if let Ok(parsed) = serde_json::from_str::<WebsocketMessage>(&payload) {
                        if let WebsocketMessage::TradeStateUpdate { offers, user_acted, status } = parsed {
                            if let Some(alice_map) = offers.get(&alice_address) {
                                received_update_ws2 = true;
                                // Check the data if needed:
                                // let maybe_alice = offers.get(&alice_address);
                                // assert!(maybe_alice.is_some(), "No 'Alice' user in update");
                                // let alice_map = alice.unwrap();
                                assert_eq!(alice_map.get(&token_mint), Some(&dec!(100.1337)));
                            }
                        }
                    }
                }
            }

            // If both have received the update, break early
            if received_update_ws1 && received_update_ws2 {
                break;
            }
        }

        assert!(received_update_ws1, "ws1 did not receive TradeStateUpdate");
        assert!(received_update_ws2, "ws2 did not receive TradeStateUpdate");

        // 8. Close down websockets
        ws1.send(Message::Close(None)).await?;
        ws2.send(Message::Close(None)).await?;

        // 9. Stop server
        server.abort(); // ends the server task

        Ok(())
    }
}
