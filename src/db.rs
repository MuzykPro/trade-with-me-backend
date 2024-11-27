use rusqlite::Connection;

pub struct SqliteDbClient {
    connection: Connection,
}

impl SqliteDbClient {
    pub fn get_db_connection(&self) -> &Connection {
        &self.connection
    }

    pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
        let connection = Connection::open("trade_with_me.db")?;

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

        Ok(SqliteDbClient { connection })
    }
}
