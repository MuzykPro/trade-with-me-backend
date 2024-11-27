
use rusqlite::params;

use crate::db::{self, SqliteDbClient};

pub struct MetadataRepository {
    db: SqliteDbClient,
}

impl MetadataRepository {
    pub fn new(db_client: SqliteDbClient) -> Self {
        MetadataRepository { db: db_client }
    }

    fn insert_metadata(&self, metadata: MetadataEntity) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.db.get_db_connection();

        conn.execute("INSERT INTO metadata (mint_address, name, symbol, uri, is_nft, image) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
         params![metadata.mint_address, metadata.name, metadata.symbol, metadata.uri, metadata.is_nft, metadata.image])?;

         Ok(())
    }

    fn get_metadata(&self, mint_address: &str) -> Result<MetadataEntity, Box<dyn std::error::Error>> {
        let conn = self.db.get_db_connection();

        Ok(conn.query_row("SELECT mint_address, name, symbol, uri, is_nft, image FROM metadata WHERE mint_address= ?1", 
                params![mint_address.to_string()],
                |row| {
                    Ok(MetadataEntity {
                        mint_address: row.get(0)?,
                        name: row.get(1)?,
                        symbol: row.get(2)?,
                        uri: row.get(3)?,
                        is_nft: row.get(4)?,
                        image: row.get(5)?,                
                    })
                })?)
    
    }

    fn get_all_saved_mint_addresses(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let conn = self.db.get_db_connection();

        let mut stmt = conn.prepare("SELECT mint_address FROM metadata")?;
        let mut rows= stmt.query([])?;
        let mut mint_addresses: Vec<String> = Vec::new();
        while let Some(row) = rows.next()?  {
            mint_addresses.push(row.get(0)?);
        }

        Ok(mint_addresses)
    }
}


fn save_metadata() {}
struct MetadataEntity {
    pub mint_address: String,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub is_nft: bool,
    pub image: Vec<u8>,
}
