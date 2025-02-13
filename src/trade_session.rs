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
use strum_macros::Display;
use crate::chain_context::ChainContext;
use crate::token_amount_cache::TokenAmountCache;
use crate::trade_websocket::WebsocketMessage;
use crate::transaction_service::{self, TransactionService};
pub type SessionId = Uuid;
pub type ConnectionId = Uuid;

pub struct SharedSessions<T: ChainContext> {
    internal: Mutex<HashMap<SessionId, TradeSession>>,
    token_amount_cache: Arc<TokenAmountCache>,
    transaction_service: Arc<TransactionService<T>>,
}
impl<T: ChainContext> SharedSessions<T> {
    pub fn new(token_amount_cache: Arc<TokenAmountCache>, transaction_service: Arc<TransactionService<T>>) -> Self {
        SharedSessions {
            internal: Mutex::default(),
            token_amount_cache,
            transaction_service,
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
                    user_acted: trade_session.state.user_acted.clone(),
                    status: trade_session.state.status.to_string(),
                });
            }
        }
    }

    pub fn add_tokens_offer(
        &self,
        session_id: &SessionId,
        user_address: &str,
        token_mint: String,
        token_amount: Decimal,
    ) -> Result<()> {
        if token_amount <= dec!(0) {
            return Ok(());
        }
        
        let mut sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get_mut(session_id) {
            if !matches!(trade_session.state.status,
                TradeStatus::Trading | TradeStatus::OneUserAccepted
            ) {
                return Err(Error::msg(format!("Invalid action for current trade session state")));
            }
            let token_amounts = self.token_amount_cache.get_token_amounts(user_address);
            let available_tokens = token_amounts.map_or_else(
                || dec!(0),
                |amounts| {
                    amounts
                        .get(&token_mint)
                        .map_or_else(|| dec!(0), |amount| amount.to_owned())
                },
            );

            let mut new_state_items = (*trade_session.state.items).clone();
            if let Some(trade_items) = new_state_items.get_mut(user_address) {
                trade_items
                    .entry(token_mint)
                    .and_modify(|amount| {
                        *amount = cmp::min(*amount + token_amount, available_tokens)
                    })
                    .or_insert(cmp::min(token_amount, available_tokens));
                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                    user_acted: None,
                    status: TradeStatus::Trading
                };
            } else if trade_session.state.items.len() == 2 {
                return Err(Error::msg(
                    "There are already 2 users involved in this trade",
                ));
            } else {
                new_state_items.insert(
                    String::from(user_address),
                    HashMap::from([(token_mint, cmp::min(token_amount, available_tokens))]),
                );
                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                    user_acted: None,
                    status: TradeStatus::Trading,
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
        user_address: &str,
        token_mint: String,
        token_amount: Decimal,
    ) -> Result<()> {
        if token_amount <= dec!(0) {
            return Ok(());
        }
        let mut sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get_mut(session_id) {
            if !matches!(trade_session.state.status,
                TradeStatus::Trading | TradeStatus::OneUserAccepted
            ) {
                return Err(Error::msg(format!("Invalid action for current trade session state")));
            }
            let mut new_state_items = (*trade_session.state.items).clone();
            if let Some(trade_items) = new_state_items.get_mut(user_address) {
                trade_items.entry(token_mint.clone()).and_modify(|amount| {
                    *amount = if token_amount >= *amount {
                        dec!(0)
                    } else {
                        *amount - token_amount
                    }
                });
                if let Some(a) = trade_items.get(&token_mint) {
                    if *a == dec!(0) {
                        trade_items.remove(&token_mint);
                    }
                }

                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                    user_acted: None,
                    status: TradeStatus::Trading,
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

    pub fn accept_trade(&self, session_id: &SessionId, user_address: &str) -> Result<()> {
        let mut sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get_mut(session_id) {
            if !matches!(trade_session.state.status,
                TradeStatus::Trading | TradeStatus::OneUserAccepted
            ) {
                return Err(Error::msg(format!("Invalid action for current trade session state")));
            }
            if let Some(user_accepted) = &trade_session.state.user_acted {
                if *user_accepted != user_address {
                    trade_session.state.user_acted = None;
                trade_session.state.status = TradeStatus::Accepted;
                }
            } else {
                trade_session.state.user_acted = Some(String::from(user_address));
                trade_session.state.status = TradeStatus::OneUserAccepted;
            }
            
        } else {
            return Err(Error::msg(format!("Session {} not found", session_id)));
        }
        Ok(())
    }

    pub fn get_transaction_to_sign(&self, session_id: &SessionId, ) -> Result<()> {
        Ok(())
    }
    pub fn sign_transaction(&self, session_id: &SessionId, signature: String) -> Result<()> {
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
    pub user_acted: Option<String>,
    pub status: TradeStatus,
}

#[derive(Clone, Debug, Display, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TradeStatus {
    #[default]
    Trading,
    OneUserAccepted,
    Accepted,
    TransactionCreated,
    OneUserSigned,
    TransactionSent
}

#[cfg(test)]
mod tests {
    use crate::{chain_context::TestChainContext, token_amount_cache};

    use super::*;
    use solana_sdk::transaction;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_accept_trade_only_possible_in_trading_or_oneuseraccepted_status() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let user_address1 = String::from("Alice");

        token_amount_cache.insert_token_amounts(
            user_address1.clone(),
            HashMap::from([("TokenA".to_string(), dec!(0.6))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address1,
            "TokenA".to_string(),
            dec!(0.1001),
        );
        assert!(result.is_ok());

        let _ = shared.accept_trade(&session_id, &user_address1);
       

        // states that should not allow changing token offers
        for trade_status in vec![TradeStatus::Accepted, TradeStatus::TransactionCreated, TradeStatus::OneUserSigned, TradeStatus::TransactionSent]
        {
            //change trade status
            {
                let mut sessions = shared.internal.lock().unwrap();
                let session = sessions.get_mut(&session_id).expect("Session not found");
                session.state.status = trade_status;
            }
            
            let result = shared.accept_trade(&session_id, &user_address1);
            assert!(result.is_err());
    
        }
    }

    #[tokio::test]
    async fn test_trade_must_be_mutable_only_in_trading_or_oneuseraccepted_status() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let user_address1 = String::from("Alice");
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));

        token_amount_cache.insert_token_amounts(
            user_address1.clone(),
            HashMap::from([("TokenA".to_string(), dec!(0.6))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address1,
            "TokenA".to_string(),
            dec!(0.1001),
        );
        assert!(result.is_ok());

        // states that allow mutability
        for trade_status in vec![TradeStatus::Trading, TradeStatus::OneUserAccepted]
        {
            //change trade status
            {
                let mut sessions = shared.internal.lock().unwrap();
                let session = sessions.get_mut(&session_id).expect("Session not found");
                session.state.status = trade_status;
            }
            
            let result = shared.add_tokens_offer(
                &session_id,
                &user_address1,
                "TokenA".to_string(),
                dec!(0.1001),
            );
            assert!(result.is_ok());

            let result = shared.withdraw_tokens(
                &session_id,
                &user_address1,
                "TokenA".to_string(),
                dec!(0.0501),
            );
            assert!(result.is_ok());
        }

        // states that should not allow changing token offers
        for trade_status in vec![TradeStatus::Accepted, TradeStatus::TransactionCreated, TradeStatus::OneUserSigned, TradeStatus::TransactionSent]
        {
            //change trade status
            {
                let mut sessions = shared.internal.lock().unwrap();
                let session = sessions.get_mut(&session_id).expect("Session not found");
                session.state.status = trade_status;
            }
            
            let result = shared.add_tokens_offer(
                &session_id,
                &user_address1,
                "TokenA".to_string(),
                dec!(0.1001),
            );
            assert!(result.is_err());

            let result = shared.withdraw_tokens(
                &session_id,
                &user_address1,
                "TokenA".to_string(),
                dec!(0.0801),
            );
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_second_user_accepts() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let user_address1 = String::from("Alice");
        let user_address2 = String::from("Bob");
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));


        token_amount_cache.insert_token_amounts(
            user_address1.clone(),
            HashMap::from([("TokenA".to_string(), dec!(0.6))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address1,
            "TokenA".to_string(),
            dec!(0.1001),
        );
        assert!(result.is_ok());

        let _ = shared.accept_trade(&session_id, &user_address1);
        let _ = shared.accept_trade(&session_id, &user_address2);


        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get("Alice")
                .expect("Alice not found in state");

            assert_eq!(session.state.user_acted, None);
            assert_eq!(session.state.status, TradeStatus::Accepted);
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(0.1001)
            );
        }
    }

    #[tokio::test]
    async fn test_accept_trade() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let user_address = String::from("Alice");
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));

        token_amount_cache.insert_token_amounts(
            user_address.clone(),
            HashMap::from([("TokenA".to_string(), dec!(0.6))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address,
            "TokenA".to_string(),
            dec!(0.1001),
        );
        assert!(result.is_ok());

        shared.accept_trade(&session_id, &user_address);

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get("Alice")
                .expect("Alice not found in state");

            assert_eq!(session.state.user_acted, Some(user_address));
            assert_eq!(session.state.status, TradeStatus::OneUserAccepted);
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(0.1001)
            );
        }
    }

    #[tokio::test]
    async fn test_offering_token_should_revert_accept() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let user_address = String::from("Alice");
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));

        token_amount_cache.insert_token_amounts(
            user_address.clone(),
            HashMap::from([("TokenA".to_string(), dec!(15))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address,
            "TokenA".to_string(),
            dec!(13.37),
        );
        assert!(result.is_ok());

        shared.accept_trade(&session_id, &user_address);

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get("Alice")
                .expect("Alice not found in state");

            assert_eq!(session.state.user_acted, Some(user_address.clone()));
            assert_eq!(session.state.status, TradeStatus::OneUserAccepted);
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(13.37)
            );
        }

        let result = shared.add_tokens_offer(
            &session_id,
            &user_address,
            "TokenA".to_string(),
            dec!(1.00),
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

            assert_eq!(session.state.user_acted, None);
            assert_eq!(session.state.status, TradeStatus::Trading);

            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(14.37)
            );
        }

    }

    #[tokio::test]
    async fn test_withdrawing_token_should_revert_accept() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let user_address = String::from("Alice");
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));

        token_amount_cache.insert_token_amounts(
            user_address.clone(),
            HashMap::from([("TokenA".to_string(), dec!(14))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address,
            "TokenA".to_string(),
            dec!(13.37),
        );
        assert!(result.is_ok());

        shared.accept_trade(&session_id, &user_address);

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get("Alice")
                .expect("Alice not found in state");

            assert_eq!(session.state.user_acted, Some(user_address.clone()));
            assert_eq!(session.state.status, TradeStatus::OneUserAccepted);
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(13.37)
            );
        }

        let result = shared.withdraw_tokens(
            &session_id,
            &user_address,
            "TokenA".to_string(),
            dec!(0.37),
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

            assert_eq!(session.state.user_acted, None);
            assert_eq!(session.state.status, TradeStatus::Trading);
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(13.0)
            );
        }

    }

    #[tokio::test]
    async fn test_second_user_accept_should_move_trade_state_to_accepted() {
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let user_address = String::from("Alice");
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));

        token_amount_cache.insert_token_amounts(
            user_address.clone(),
            HashMap::from([("TokenA".to_string(), dec!(0.6))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address,
            "TokenA".to_string(),
            dec!(0.1001),
        );
        assert!(result.is_ok());

        shared.accept_trade(&session_id, &user_address);

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get("Alice")
                .expect("Alice not found in state");

            assert_eq!(session.state.user_acted, Some(user_address));
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(0.1001)
            );
        }
    }

    #[tokio::test]
    async fn test_add_client() {
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
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
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
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
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
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
                WebsocketMessage::TradeStateUpdate { offers: _, user_acted: _, status: _ },
                WebsocketMessage::TradeStateUpdate { offers: _ , user_acted: _, status: _},
            ) => {
                // Just ensuring that both got the correct variant
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[tokio::test]
    async fn test_add_tokens_offer() {
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let user_address = String::from("Alice");
        token_amount_cache.insert_token_amounts(
            user_address.clone(),
            HashMap::from([("TokenA".to_string(), dec!(0.6))]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        // Add tokens for user "Alice"
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address,
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
                .get(&user_address)
                .expect("Alice not found in state");
            assert_eq!(
                *alice_tokens.get("TokenA").expect("TokenA not found"),
                dec!(0.1001)
            );
        }
        // Add more tokens for Alice, same mint
        let result = shared.add_tokens_offer(
            &session_id,
            &user_address,
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
                .get(&user_address)
                .expect("Alice not found in state");
            assert_eq!(
                *updated_alice_tokens
                    .get("TokenA")
                    .expect("TokenA not found"),
                dec!(0.6)
            );
        }

        // Add second user "Bob"
        let result = shared.add_tokens_offer(&session_id, "Bob", "TokenB".to_string(), dec!(10));
        assert!(result.is_ok());

        // Try adding a third user should fail because we have a 2-users limit
        let result = shared.add_tokens_offer(&session_id, "Charlie", "TokenC".to_string(), dec!(5));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_withdraw_tokens() {
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let user_address = String::from("Alice");
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
                user_acted: None,
                status: TradeStatus::Trading
            };
            sessions.insert(session_id, session);
        }

        // Withdraw 50 tokens from Alice's TokenA
        let result = shared.withdraw_tokens(
            &session_id,
            &user_address,
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
            &user_address,
            "TokenA".to_string(),
            dec!(100),
        );
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session.state.items.get("Alice").expect("Alice not found");
            assert_eq!(*alice_tokens, HashMap::new());
        }

        // Withdrawing a token that does not exist
        let result: std::result::Result<(), Error> = shared.withdraw_tokens(
            &session_id,
            &user_address,
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

    #[tokio::test]
    async fn add_more_tokens_than_available() {
        let user_address = "Alice";
        let token_mint = "TokenA";
        let available_tokens = dec!(10);
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts(
            user_address.to_owned(),
            HashMap::from([(token_mint.to_string(), available_tokens)]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(12));
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get(user_address)
                .expect("Alice not found in state");
            assert_eq!(
                *alice_tokens.get(token_mint).expect("TokenA not found"),
                available_tokens
            );
        }
    }

    #[tokio::test]
    async fn add_more_tokens_than_available_multiple_times() {
        let user_address = "Alice";
        let token_mint = "TokenA";
        let available_tokens = dec!(10);
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts(
            user_address.to_owned(),
            HashMap::from([(token_mint.to_string(), available_tokens)]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(4));
        assert!(result.is_ok());
        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(4));
        assert!(result.is_ok());
        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(4));
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get(user_address)
                .expect("Alice not found in state");
            assert_eq!(
                *alice_tokens.get(token_mint).expect("TokenA not found"),
                available_tokens
            );
        }
    }

    #[tokio::test]
    async fn add_negative_amount_of_tokens() {
        let user_address = "Alice";
        let token_mint = "TokenA";
        let available_tokens = dec!(10);
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts(
            user_address.to_owned(),
            HashMap::from([(token_mint.to_string(), available_tokens)]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(4));
        assert!(result.is_ok());
        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(4));
        assert!(result.is_ok());
        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(-4));
        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get(user_address)
                .expect("Alice not found in state");
            assert_eq!(
                *alice_tokens.get(token_mint).expect("TokenA not found"),
                dec!(8)
            );
        }
    }

    #[tokio::test]
    async fn add_then_withdraw_negative_amount() {
        let user_address = "Alice";
        let token_mint = "TokenA";
        let available_tokens = dec!(10);
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts(
            user_address.to_owned(),
            HashMap::from([(token_mint.to_string(), available_tokens)]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(4));
        assert!(result.is_ok());
        let result = shared.withdraw_tokens(
            &session_id,
            &user_address,
            token_mint.to_string(),
            dec!(-4),
        );

        assert!(result.is_ok());

        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get(user_address)
                .expect("Alice not found in state");
            assert_eq!(
                *alice_tokens.get(token_mint).expect("TokenA not found"),
                dec!(4)
            );
        }
    }

    #[tokio::test]
    async fn withdraw_not_offered_tokens() {
        let user_address = "Alice";
        let token_mint = "TokenA";
        let available_tokens = dec!(10);
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts(
            user_address.to_owned(),
            HashMap::from([(token_mint.to_string(), available_tokens)]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        let result = shared.withdraw_tokens(
            &session_id,
            &user_address,
            token_mint.to_string(),
            dec!(4),
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn withdraw_below_zero() {
        let user_address = "Alice";
        let token_mint = "TokenA";
        let available_tokens = dec!(10);
        let transaction_service = Arc::new(TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{})));
        let token_amount_cache = Arc::new(TokenAmountCache::init());
        token_amount_cache.insert_token_amounts(
            user_address.to_owned(),
            HashMap::from([(token_mint.to_string(), available_tokens)]),
        );
        let shared = SharedSessions::new(token_amount_cache, transaction_service);
        let session_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let (tx, _rx) = mpsc::channel(10);
        shared.add_client(session_id, connection_id, tx);

        let result =
            shared.add_tokens_offer(&session_id, &user_address, token_mint.to_string(), dec!(4));
        assert!(result.is_ok());

        let result = shared.withdraw_tokens(
            &session_id,
            &user_address,
            token_mint.to_string(),
            dec!(3),
        );
        assert!(result.is_ok());

        let result = shared.withdraw_tokens(
            &session_id,
            &user_address,
            token_mint.to_string(),
            dec!(3),
        );
        assert!(result.is_ok());
        let result = shared.withdraw_tokens(
            &session_id,
            &user_address,
            token_mint.to_string(),
            dec!(3),
        );
        assert!(result.is_ok());

        //should delete tokens state if amount drops to zero
        {
            let sessions = shared.internal.lock().unwrap();
            let session = sessions.get(&session_id).expect("Session not found");
            let alice_tokens = session
                .state
                .items
                .get(user_address)
                .expect("Alice not found in state");
            assert_eq!(*alice_tokens, HashMap::new());
        }
    }
    //withdraw negative amount of tokens
    //withdraw negative amount of tokens, exceeding available
    //add tokens, then withdraw negative amount of tokens that exceeds available tokens
}
