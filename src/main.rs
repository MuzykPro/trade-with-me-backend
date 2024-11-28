use std::{cell::RefCell, sync::Arc};

use db::SqliteDbClient;
use metadata_cache::MetadataCache;
use metadata_repository::MetadataRepository;
use routes::{get_router, AppState};
use solana_client::nonblocking::rpc_client::RpcClient;
use token_service::TokenService;
use tokio::sync::Mutex;

pub mod metadata_cache;
pub mod routes;
pub mod token_service;
pub mod db;
pub mod metadata_repository;

// token holder address: 87UGBXfeuCaMyxNnCD3a9Wcbjc5C8c34hbKEBUfc2F86
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sqlite_db_client = SqliteDbClient::init()?;
    let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let rpc_client = Arc::new(RpcClient::new(rpc_url));

    let metadata_repository = MetadataRepository::new(sqlite_db_client);
    let metadata_cache = MetadataCache::init(metadata_repository, Arc::clone(&rpc_client))?;
    let token_service = TokenService::new(metadata_cache, Arc::clone(&rpc_client));
    let app_state = AppState {
        token_service: Arc::new(token_service)
    };
    let router = get_router(Arc::new(app_state));


    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();
    Ok(())
}
