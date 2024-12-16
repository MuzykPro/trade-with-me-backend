use std::error::Error;

use uuid::Uuid;

use crate::trade_repository::{NewTrade, TradeRepository, TradeStatus};

pub struct TradeService {
    trade_repository: TradeRepository
}

impl TradeService {
    pub fn new(trade_repository: TradeRepository) -> Self {
        TradeService {
            trade_repository
        }
    }

    pub fn create_trade_session(&self, initiator_address: &str) -> Result<Uuid, Box<dyn Error>> {
        self.trade_repository.insert_trade(NewTrade {
            initiator: initiator_address.to_string(),
            counterparty: None,
            status: TradeStatus::Created.as_str().to_string(),
            status_details: None
        })
    }
}