use std::sync::Arc;

use anyhow::{Error, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::hash::Hash;

pub trait ChainContext {
    async fn get_latest_blockhash(&self) -> Result<Hash>;
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
        self.rpc_client.get_latest_blockhash().await.map_err(anyhow::Error::from)
    }
}

#[cfg(test)]
pub struct TestChainContext {

}

#[cfg(test)]
impl ChainContext for TestChainContext {
    async fn get_latest_blockhash(&self) -> Result<Hash> {
        Ok(Hash::default())
    }
}