use std::{str::FromStr, sync::Arc};

use anyhow::{Error, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{hash::Hash, pubkey::Pubkey};

pub trait ChainContext {
    fn get_latest_blockhash(&self) -> impl std::future::Future<Output = Result<Hash>> + std::marker::Send;
    fn get_trade_with_me_program_id(&self) -> Pubkey;
}

pub struct MainnetChainContext {
    pub rpc_client: Arc<RpcClient>,
}

impl MainnetChainContext {
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }
}

impl ChainContext for MainnetChainContext {
    async fn get_latest_blockhash(&self) -> Result<Hash> {
        self.rpc_client
            .get_latest_blockhash()
            .await
            .map_err(anyhow::Error::from)
    }

    fn get_trade_with_me_program_id(&self) -> Pubkey {
        Pubkey::from_str("DMnLeeL2qJQdWHDDnXKTyRie7o1kNvKqg74UYEqzHqgq").unwrap()
    }
}

#[cfg(test)]
pub struct TestChainContext {}

#[cfg(test)]
impl ChainContext for TestChainContext {
    async fn get_latest_blockhash(&self) -> Result<Hash> {
        Ok(Hash::default())
    }
    fn get_trade_with_me_program_id(&self) -> Pubkey {
        Pubkey::from_str("DMnLeeL2qJQdWHDDnXKTyRie7o1kNvKqg74UYEqzHqgq").unwrap()
    }
}
