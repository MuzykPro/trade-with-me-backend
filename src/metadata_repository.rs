
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use crate::db::PostgreSqlClient;
use crate::schema::metadata;
use crate::schema::metadata::dsl::metadata as metadata_table;
use crate::schema::metadata::dsl::mint_address;
pub struct MetadataRepository {
    db: PostgreSqlClient,
}

impl MetadataRepository {
    pub fn new(db_client: PostgreSqlClient) -> Self {
        MetadataRepository { db: db_client }
    }

    pub fn insert_metadata(&self, metadata_entity: &MetadataEntity) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.db.get_db_connection()?;
        diesel::insert_into(metadata_table)
            .values(metadata_entity)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_metadata(&self, mint_addr: &str) -> Result<MetadataEntity, Box<dyn std::error::Error>> {
        let mut conn = self.db.get_db_connection()?;
        Ok(metadata_table
                    .filter(mint_address.eq(mint_addr))
                    .first::<MetadataEntity>(&mut conn)?)
    }

    pub fn get_all_saved_mint_addresses(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut conn = self.db.get_db_connection()?;
        Ok(metadata_table
            .select(mint_address)
            .load::<String>(&mut conn)?)

    }
}

#[derive(Debug, Queryable, Insertable, Serialize, Deserialize)]
#[diesel(table_name = metadata)]
pub struct MetadataEntity {
    pub mint_address: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub uri: Option<String>,
    pub image: Option<Vec<u8>>,
}
