use std::sync::Arc;
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use anyhow::Result;
use image::ImageFormat;
use mpl_token_metadata::accounts::Metadata;
use mpl_token_metadata::ID as TOKEN_METADATA_PROGRAM_ID;
use reqwest::get;
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::RwLock;

use crate::metadata_repository::{MetadataEntity, MetadataRepository};

pub struct MetadataCache {
    known_mint_addresses: RwLock<HashSet<String>>,
    metadata_repository: MetadataRepository,
    rpc_client: Arc<RpcClient>
}

impl MetadataCache {
    pub fn init(metadata_repository: MetadataRepository, rpc_client: Arc<RpcClient>) -> Result<Self, Box<dyn std::error::Error>> {
        let known_mint_addresses = metadata_repository.get_all_saved_mint_addresses()?;
        Ok(MetadataCache {
                    known_mint_addresses: RwLock::new(known_mint_addresses.into_iter().collect()),
                    metadata_repository,
                    rpc_client
                })
    }
    pub async fn get_token_metadata(&self, mint_address: &str) -> Result<MetadataEntity> {
        if self.known_mint_addresses.read().await.contains(mint_address) {
            match self.metadata_repository.get_metadata(mint_address) {
                Ok(result) => return Ok(result),
                Err(_) => eprintln!("Unable to fetch metadata from DB")
            };
        } 

        let metaplex_metadata = self.fetch_token_metadata(mint_address).await?;
        let resized_image = MetadataCache::follow_uri_to_get_image(&metaplex_metadata.uri)
        .await
        .and_then(|image| MetadataCache::resize_image(&image))
        .unwrap_or_default();

        let new_metadata = MetadataEntity {
            mint_address: mint_address.to_string(),
            symbol: metaplex_metadata.symbol.trim_end_matches(char::from(0)).to_string(),
                        name: metaplex_metadata.name.trim_end_matches(char::from(0)).to_string(),
                        uri: metaplex_metadata.uri.trim_end_matches(char::from(0)).to_string(),
            image: resized_image,
        };
        self.known_mint_addresses.write().await.insert(mint_address.to_string());
        self.metadata_repository.insert_metadata(&new_metadata);
        Ok(new_metadata)
    }

    async fn fetch_token_metadata(&self, mint_address: &str) -> Result<Metadata> {
        let mint_pubkey = Pubkey::try_from(mint_address)?;
        let metadata_pubkey = MetadataCache::derive_metadata_account(&mint_pubkey);
        let account_data = self.rpc_client.get_account_data(&metadata_pubkey).await?;
        let metadata: Metadata = Metadata::from_bytes(&account_data)?;
        let image_maybe = MetadataCache::follow_uri_to_get_image(&metadata.uri).await;
        if let Some(image) = image_maybe {
            let resized_maybe = MetadataCache::resize_image(&image);
            if let Some(resized) = resized_maybe {
                MetadataCache::save_image(&resized, &metadata.mint.to_string());
            }
        }
        Ok(metadata)
    }
    
    fn derive_metadata_account(mint_account: &Pubkey) -> Pubkey {
        let seeds = &[
            "metadata".as_bytes(),
            TOKEN_METADATA_PROGRAM_ID.as_ref(),
            mint_account.as_ref(),
        ];
        let (metadata_pubkey, _) = Pubkey::find_program_address(seeds, &TOKEN_METADATA_PROGRAM_ID);
        metadata_pubkey
    }
    
    async fn follow_uri_to_get_image(uri: &str) -> Option<Vec<u8>> {
        //uri usually should contain json with "image": "image url" so it should be first way we do it
    
        let uri_response = get(uri).await.ok();
        if let Some(response) = uri_response {
            if response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map_or(false, |v| v.contains("application/json"))
            {
                let image_uri = response.text()
                .await
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|json| json["image"].as_str().map(|r|r.to_string()));
    
                if let Some(image_url) = image_uri {
                    return MetadataCache::try_fetch_image(&image_url).await;
                } else {
                    return None;
                }
            }
        } else {
            return None;
        };
    
        None
    }
    
    async fn try_fetch_image(image_url: &str) -> Option<Vec<u8>> {
        let image_response = get(image_url).await.ok();
        if let Some(response) = image_response {
            return response.bytes().await.ok().map(|bytes|bytes.to_vec());
        } else {
            return None;
        }    
    }
    
    fn resize_image(image: &[u8]) -> Option<Vec<u8>> {
        image::load_from_memory(image)
        .map(|i| i.resize_exact(64, 64, image::imageops::FilterType::Lanczos3))
        .map(|resized| {
            let mut buf = Cursor::new(Vec::new());
            resized.write_to(&mut buf, ImageFormat::Png).ok();
            buf.into_inner()
        })
        .ok()
    }
    
    fn save_image(image: &[u8], mint_address: &str) -> Result<()> {
        let folder_path = format!("{}/{}", "metadata/tokens", mint_address);
        let file_name = "token_icon_64_64.png";
    
        if !Path::new(&folder_path).exists() {
            fs::create_dir_all(&folder_path)?; // Creates all missing parent directories
            println!("Folder '{}' created.", folder_path);
        } else {
            println!("Folder '{}' already exists.", folder_path);
        }
    
        let file_path = format!("{}/{}", folder_path, file_name);
    
        // Write the image data to the file
        if let Err(e) = fs::write(&file_path, &image) {
            eprintln!("Failed to write image to file: {}", e);
        } else {
            println!("Image saved successfully to {}", file_path);
        }
        Ok(())
    }
}
