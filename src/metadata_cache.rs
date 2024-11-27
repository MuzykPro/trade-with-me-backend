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

pub async fn fetch_token_metadata(client: &RpcClient, mint_address: &str) -> Result<Metadata> {
    let mint_pubkey = Pubkey::try_from(mint_address)?;
    let metadata_pubkey = derive_metadata_account(&mint_pubkey);
    let account_data = client.get_account_data(&metadata_pubkey).await?;
    let metadata: Metadata = Metadata::from_bytes(&account_data)?;
    let image_maybe = follow_uri_to_get_image(&metadata.uri).await;
    if let Some(image) = image_maybe {
        let resized_maybe = resize_image(&image);
        if let Some(resized) = resized_maybe {
            save_image(&resized, &metadata.mint.to_string());
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
                return try_fetch_image(&image_url).await;
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