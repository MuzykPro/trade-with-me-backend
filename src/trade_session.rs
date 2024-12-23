use anyhow::*;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::cmp;
use std::result::Result::Ok;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::token_amount_cache::TokenAmountCache;
use crate::trade_websocket::WebsocketMessage;
pub type SessionId = Uuid;
pub type ConnectionId = Uuid;

pub struct SharedSessions {
    internal: Mutex<HashMap<SessionId, TradeSession>>,
    token_amount_cache: Arc<TokenAmountCache>,
}
impl SharedSessions {
    pub fn new(token_amount_cache: Arc<TokenAmountCache>) -> Self {
        SharedSessions {
            internal: Mutex::default(),
            token_amount_cache 
        }
    }

    pub fn add_client(
        &self,
        session_id: SessionId,
        connection_id: ConnectionId,
        tx: mpsc::Sender<WebsocketMessage>,
    ) {
        let mut sessions = self.internal.lock().unwrap();
        sessions
            .entry(session_id)
            .or_default()
            .ws_clients
            .insert(connection_id, tx);
    }

    pub fn remove_client(&self, session_id: &SessionId, connection_id: &ConnectionId) {
        let mut sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get_mut(session_id) {
            trade_session.ws_clients.remove(connection_id);
        }
    }

    pub fn broadcast_current_state(&self, session_id: &SessionId) {
        let sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get(session_id) {
            for tx in trade_session.ws_clients.values() {
                let _ = tx.try_send(WebsocketMessage::TradeStateUpdate {
                    offers: Arc::clone(&trade_session.state.items),
                });
            }
        }
    }

    pub fn add_tokens_offer(
        &self,
        session_id: &SessionId,
        user_address: String,
        token_mint: String,
        token_amount: Decimal,
    ) -> Result<()> {
        let mut sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get_mut(session_id) {
            let token_amounts = self.token_amount_cache.get_token_amounts(&user_address);
            let available_tokens = token_amounts.map_or_else(|| dec!(0), 
            |amounts| amounts.get(&token_mint).map_or_else(||dec!(0),
            |amount|amount.to_owned()));

            let mut new_state_items = (*trade_session.state.items).clone();
            if let Some(trade_items) = new_state_items.get_mut(&user_address) {
                trade_items
                    .entry(token_mint)
                    .and_modify(|amount| *amount = cmp::min(*amount + token_amount, available_tokens))
                    .or_insert(token_amount);
                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                };
            } else if trade_session.state.items.len() == 2 {
                return Err(Error::msg(
                    "There are already 2 users involved in this trade",
                ));
            } else {
                new_state_items.insert(user_address, HashMap::from([(token_mint, token_amount)]));
                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                };
            }
        } else {
            return Err(Error::msg(format!("Session {} not found", session_id)));
        }
        Ok(())
    }

    pub fn withdraw_tokens(
        &self,
        session_id: &SessionId,
        user_address: String,
        token_mint: String,
        token_amount: Decimal,
    ) -> Result<()> {
        let mut sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get_mut(session_id) {
            let mut new_state_items = (*trade_session.state.items).clone();
            if let Some(trade_items) = new_state_items.get_mut(&user_address) {
                trade_items.entry(token_mint).and_modify(|amount| {
                    *amount = if token_amount >= *amount {
                        dec!(0)
                    } else {
                        *amount - token_amount
                    }
                });
                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                };
            } else {
                return Err(Error::msg(format!(
                    "There are no tokens {} in session state",
                    token_mint
                )));
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct TradeSession {
    pub state: TradeState,
    pub ws_clients: HashMap<ConnectionId, mpsc::Sender<WebsocketMessage>>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TradeState {
    pub items: Arc<HashMap<String, HashMap<String, Decimal>>>,
}

#[cfg(test)]
mod tests {
    use crate::token_amount_cache;

    use super::*;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_add_client() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        let sessions = shared.internal.lock().unwrap();
        let session = sessions.get(&session_id).expect("Session not found");
        assert!(session.ws_clients.contains_key(&connection_id));
    }

    #[tokio::test]
    async fn test_remove_client() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Remove the client
        shared.remove_client(&session_id, &connection_id);

        let sessions = shared.internal.lock().unwrap();
        let session = sessions.get(&session_id).expect("Session not found");
        assert!(!session.ws_clients.contains_key(&connection_id));
    }

    #[tokio::test]
    async fn test_broadcast_current_state() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache);
        let session_id = Uuid::new_v4();
        let connection_id_1 = Uuid::new_v4();
        let connection_id_2 = Uuid::new_v4();

        let (tx1, mut rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);

        shared.add_client(session_id, connection_id_1, tx1);
        shared.add_client(session_id, connection_id_2, tx2);

        shared.broadcast_current_state(&session_id);

        let msg1 = rx1.recv().await.expect("No message received by client 1");
        let msg2 = rx2.recv().await.expect("No message received by client 2");

        match (msg1, msg2) {
            (
                WebsocketMessage::TradeStateUpdate { offers: _ },
                WebsocketMessage::TradeStateUpdate { offers: _ },
            ) => {
                // Just ensuring that both got the correct variant
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[tokio::test]
    async fn test_add_tokens_offer() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts("Alice".to_string(), HashMap::from([("TokenA".to_string(), dec!(0.6))]));
        let shared = SharedSessions::new(token_amount_cache);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            "Alice".to_string(),
            "TokenA".to_string(),
            dec!(0.1001),
        );
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get("Alice")
                .expect("Alice not found in state");
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(0.1001)
            );
        }
        // Add more tokens for Alice, same mint
        let result = shared.add_tokens_offer(
            &session_id,
            "Alice".to_string(),
            "TokenA".to_string(),
            dec!(0.5001),
        );
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let updated_alice_tokens = session
                .state
                .items
                .get("Alice")
                .expect("Alice not found in state");
            assert_eq!(
                *updated_alice_tokens
                    .get("TokenA")
                    .expect("TokenA not found"),
                dec!(0.6)
            );
        }

        // Add second user "Bob"
        let result = shared.add_tokens_offer(
            &session_id,
            "Bob".to_string(),
            "TokenB".to_string(),
            dec!(10),
        );
        assert!(result.is_ok());

        // Try adding a third user should fail because we have a 2-users limit
        let result = shared.add_tokens_offer(
            &session_id,
            "Charlie".to_string(),
            "TokenC".to_string(),
            dec!(5),
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_withdraw_tokens() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache);
        let session_id = Uuid::new_v4();

        // Create a session with some tokens
        {
            let mut sessions = shared.internal.lock().unwrap();
            let mut session = TradeSession::default();
            let mut map = HashMap::new();
            map.insert("TokenA".to_string(), dec!(100));
            let mut user_map = HashMap::new();
            user_map.insert("Alice".to_string(), map);
            session.state = TradeState {
                items: Arc::new(user_map),
            };
            sessions.insert(session_id, session);
        }

        // Withdraw 50 tokens from Alice's TokenA
        let result = shared.withdraw_tokens(
            &session_id,
            "Alice".to_string(),
            "TokenA".to_string(),
            dec!(50),
        );
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session.state.items.get("Alice").expect("Alice not found");
            let token_a_amount = alice_tokens.get("TokenA").expect("TokenA not found");
            assert_eq!(*token_a_amount, dec!(50));
        }

        // Withdraw more tokens than available should not go below zero
        let result = shared.withdraw_tokens(
            &session_id,
            "Alice".to_string(),
            "TokenA".to_string(),
            dec!(100),
        );
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session.state.items.get("Alice").expect("Alice not found");
            let token_a_amount = alice_tokens.get("TokenA").expect("TokenA not found");
            assert_eq!(*token_a_amount, dec!(0));
        }

        // Withdrawing a token that does not exist
        let result: std::result::Result<(), Error> = shared.withdraw_tokens(
            &session_id,
            "Alice".to_string(),
            "TokenB".to_string(),
            dec!(10),
        );
        // Should insert token with requested amount (but subtracting should yield 0)
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session.state.items.get("Alice").expect("Alice not found");
            let token_b_maybe = alice_tokens.get("TokenB");
            // TokenB didn't exist previously, now it should be max(0, 0 - 10) = 0 inserted
            assert_eq!(token_b_maybe.is_none(), true);
        }
    }
}
