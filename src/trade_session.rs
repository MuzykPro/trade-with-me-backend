use std::{
    cmp, collections::HashMap, sync::{Arc, Mutex}
};

use anyhow::*;
use std::result::Result::Ok;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::trade_websocket::WebsocketMessage;
pub type SessionId = Uuid;
pub type ConnectionId = Uuid;

#[derive(Default)]
pub struct SharedSessions {
    internal: Mutex<HashMap<SessionId, TradeSession>>,
}
impl SharedSessions {
    pub fn new() -> Self {
        SharedSessions {
            internal: Mutex::default(),
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
        token_amount: u64,
    ) -> Result<()> {
        let mut sessions = self.internal.lock().unwrap();
        if let Some(trade_session) = sessions.get_mut(session_id) {
            let mut new_state_items = (*trade_session.state.items).clone();
            if let Some(trade_items) = new_state_items.get_mut(&user_address) {
                trade_items
                    .entry(token_mint)
                    .and_modify(|amount| *amount += token_amount)
                    .or_insert(token_amount);
                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                };
            } else {
                if trade_session.state.items.len() == 2 {
                    return Err(Error::msg(
                        "There are already 2 users involved in this trade",
                    ));
                } else {
                    new_state_items
                        .insert(user_address, HashMap::from([(token_mint, token_amount)]));
                    trade_session.state = TradeState {
                        items: Arc::new(new_state_items),
                    };
                }
            }
        } else {
            return Err(Error::msg(format!("Session {} not found", session_id)));
        }
        Ok(())
    }

    pub fn withdraw_tokens(&self,
        session_id: &SessionId,
        user_address: String,
        token_mint: String,
        token_amount: u64,) -> Result<()> {
            let mut sessions = self.internal.lock().unwrap();
            if let Some(trade_session) = sessions.get_mut(session_id) {
                let mut new_state_items = (*trade_session.state.items).clone();
                if let Some(trade_items) = new_state_items.get_mut(&user_address) {
                    trade_items
                    .entry(token_mint)
                    .and_modify(|amount| *amount = cmp::max(0, *amount - token_amount))
                    .or_insert(token_amount);
                trade_session.state = TradeState {
                    items: Arc::new(new_state_items),
                };
                } else {
                    return Err(Error::msg(
                        format!("There are no tokens {} in session state", token_mint)
                    ));
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
    pub items: Arc<HashMap<String, HashMap<String, u64>>>,
}
