use std::sync::Arc;

use config::Config;
use db::PostgreSqlClient;
use env_logger::Env;
use figment::{
    providers::{Format, Yaml},
    Figment,
};
use log::info;
use metadata_cache::MetadataCache;
use metadata_repository::MetadataRepository;
use routes::{get_router, AppState};
use solana_client::nonblocking::rpc_client::RpcClient;
use token_amount_cache::TokenAmountCache;
use token_service::TokenService;
use trade_repository::TradeRepository;
use trade_service::TradeService;
use trade_session::SharedSessions;

pub mod config;
pub mod db;
pub mod metadata_cache;
pub mod metadata_repository;
pub mod routes;
pub mod schema;
pub mod token_service;
pub mod trade_repository;
pub mod trade_service;
pub mod trade_websocket;
pub mod trade_session;
pub mod token_amount_cache;
pub mod transaction_service;

// example token holder address: 87UGBXfeuCaMyxNnCD3a9Wcbjc5C8c34hbKEBUfc2F86
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config: Config = Figment::new().merge(Yaml::file("config.yaml")).extract()?;
    
    let sqlite_db_client = Arc::new(PostgreSqlClient::init(&config.postgres)?);
    let rpc_client = Arc::new(RpcClient::new(config.rpc_url));

    let metadata_repository = MetadataRepository::new(Arc::clone(&sqlite_db_client));
    let metadata_cache = MetadataCache::init(metadata_repository, Arc::clone(&rpc_client))?;
    let token_amount_cache = Arc::new(TokenAmountCache::init());
    let token_service = TokenService::new(metadata_cache, Arc::clone(&rpc_client), Arc::clone(&token_amount_cache));
    let trade_repository = TradeRepository::new(Arc::clone(&sqlite_db_client));
    let trade_service = TradeService::new(trade_repository);
    let app_state = AppState {
        token_service: Arc::new(token_service),
        trade_service: Arc::new(trade_service)
    };
    let trade_sessions = Arc::new(SharedSessions::new(Arc::clone(&token_amount_cache)));
    let router = get_router(Arc::new(app_state), trade_sessions);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Server started on port 3000");
    axum::serve(listener, router).await.unwrap();
    Ok(())
}
