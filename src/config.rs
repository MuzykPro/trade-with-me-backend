use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub postgres: PostgresConfig,
}

#[derive(Debug, Deserialize)]
pub struct PostgresConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}
