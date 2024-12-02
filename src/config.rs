use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub postgres: PostgresConfig,
}

#[derive(Debug, Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String
}
