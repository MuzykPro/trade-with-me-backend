use db::SqliteDbClient;
use routes::get_router;

pub mod metadata_cache;
pub mod routes;
pub mod tokens_fetcher;
pub mod db;
pub mod metadata_repository;

// token holder address: 87UGBXfeuCaMyxNnCD3a9Wcbjc5C8c34hbKEBUfc2F86
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sqlite_db_client = SqliteDbClient::init()?;
    let router = get_router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();
    Ok(())
}
