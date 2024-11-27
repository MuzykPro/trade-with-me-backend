use routes::get_router;

pub mod metadata_cache;
pub mod routes;
pub mod tokens_fetcher;

// token holder address: 87UGBXfeuCaMyxNnCD3a9Wcbjc5C8c34hbKEBUfc2F86
#[tokio::main]
async fn main() {
    let router = get_router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
