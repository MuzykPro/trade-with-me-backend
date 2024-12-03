use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::schema::trades::dsl::trades as trades_table;
use crate::{db::PostgreSqlClient, schema::trades};
use std::str::FromStr;
use std::sync::Arc;
pub struct TradeRepository {
    db_client: Arc<PostgreSqlClient>
}

impl TradeRepository {
    pub fn new(db_client: Arc<PostgreSqlClient>) -> Self {
        TradeRepository { db_client }
    }

    pub fn insert_trade(&self, new_trade: NewTrade) ->  Result<(), Box<dyn std::error::Error>>{
        let mut conn = self.db_client.get_db_connection()?;
        diesel::insert_into(trades_table)
            .values(new_trade)
            .execute(&mut conn)?;
        Ok(())
    }
}

#[derive(Queryable, Serialize, Deserialize, Debug)]
pub struct TradeEntity {
    pub id: Uuid,
    pub initiator: String,
    pub counterparty: Option<String>,
    pub status: String, 
    pub status_details: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[diesel(table_name = trades)]
pub struct NewTrade {
    pub initiator: String,
    pub counterparty: Option<String>,
    pub status: String,
    pub status_details: Option<serde_json::Value>,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeStatus {
    Created,
    Expired,
}

impl TradeStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TradeStatus::Created => "Created",
            TradeStatus::Expired => "Expired",
        }
    }
}

impl FromStr for TradeStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Created" => Ok(TradeStatus::Created),
            "Expired" => Ok(TradeStatus::Expired),
            _ => Err(format!("Invalid trade status: {}", s)),
        }
    }
}

impl AsRef<str> for TradeStatus {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
