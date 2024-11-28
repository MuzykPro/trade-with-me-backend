use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;

pub struct SqliteDbClient {
    pool: Pool<SqliteConnectionManager>
}

impl SqliteDbClient {
    pub fn get_db_connection(&self) -> PooledConnection<SqliteConnectionManager> {
        self.pool.get().unwrap()
    }

    pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = SqliteConnectionManager::file("trade_with_me.db");
        let pool = Pool::new(manager).expect("Failed to create pool.");

        let connection = pool.get()?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS metadata (
                mint_address TEXT PRIMARY KEY,
                name TEXT,
                symbol TEXT,
                uri TEXT,
                is_nft INTEGER,
                image BLOB
            )",
            [],
        )?;

        Ok(SqliteDbClient { pool })
    }
}
