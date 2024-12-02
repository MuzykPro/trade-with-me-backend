use diesel::{r2d2::ConnectionManager, PgConnection};
use log::info;
use r2d2::{Pool, PooledConnection};

use crate::config::PostgresConfig;

type PgPool = Pool<ConnectionManager<PgConnection>>;
pub struct PostgreSqlClient {
    pool: PgPool,
}

impl PostgreSqlClient {
    pub fn init(config: &PostgresConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let database_url = format!(
            "postgres://{}:{}@{}:{}/{}",
            config.user, config.password, config.host, config.port, config.database
        );
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = Pool::builder()
            .build(manager)
            .expect("Failed to create pool.");

        info!("Successfully connected to postgres database");

        Ok(PostgreSqlClient { pool })
    }

    pub fn get_db_connection(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<PgConnection>>, r2d2::Error> {
        self.pool.get()
    }
}
