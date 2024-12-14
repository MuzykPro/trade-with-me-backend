use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

use crate::metadata_cache::MetadataCache;

pub struct TokenService {
    metadata_cache: MetadataCache,
    rpc_client: Arc<RpcClient>,
}

impl TokenService {
    pub fn new(metadata_cache: MetadataCache, rpc_client: Arc<RpcClient>) -> Self {
        TokenService {
            metadata_cache,
            rpc_client,
        }
    }

    pub async fn fetch_tokens(
        &self,
        wallet_address: &str,
    ) -> Result<Vec<TokenAccount>, Box<dyn std::error::Error>> {
        let wallet_pubkey = Pubkey::try_from(wallet_address)?;

        let token_accounts = self
            .rpc_client
            .get_token_accounts_by_owner(
                &wallet_pubkey,
                solana_client::rpc_request::TokenAccountsFilter::ProgramId(Pubkey::try_from(
                    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                )?),
            )
            .await?;

        let mut balances: Vec<TokenAccount> = Vec::new();

        for keyed_account in token_accounts {
            if let solana_account_decoder::UiAccountData::Json(parsed_account) =
                keyed_account.account.data
            {
                if let serde_json::Value::Object(info) = parsed_account.parsed["info"].clone() {
                    let mint = info["mint"].as_str().unwrap_or_default().to_string();
                    let token_amount = &info["tokenAmount"];

                    let balance = token_amount["uiAmount"].as_f64().unwrap_or(0.0);

                    let is_nft = TokenService::is_nft(token_amount);

                    if balance > 0.0 {
                        let metadata = self.metadata_cache.get_token_metadata(&mint).await.ok();
                        balances.push(TokenAccount {
                            token_account: keyed_account.pubkey.to_string(),
                            mint,
                            balance,
                            is_nft,
                            symbol: metadata.as_ref().and_then(|m| {
                                m.symbol.as_ref()
                                    .map(|s| s.trim_end_matches(char::from(0)).to_string())
                                    .clone()
                            }),
                            name: metadata.as_ref().and_then(|m| {
                                m.name.as_ref()
                                    .map(|n| n.trim_end_matches(char::from(0)).to_string())
                                    .clone()
                            }),
                            uri: metadata.as_ref().and_then(|m| {
                                m.uri.as_ref()
                                    .map(|u| u.trim_end_matches(char::from(0)).to_string())
                                    .clone()
                            }),
                            image: metadata.as_ref().and_then(|m| {
                                m.image.as_ref().map(|i| TokenService::encode_image_to_data_url(i))
                            }),
                        });
                    }
                }
            }
        }

        Ok(balances)
    }

    fn is_nft(token_amount: &serde_json::Value) -> bool {
        let amount = token_amount["amount"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        let decimals = token_amount["decimals"].as_u64().unwrap_or(0);

        amount == 1 && decimals == 0
    }

    fn encode_image_to_data_url(image_data: &[u8]) -> String {
        if image_data.is_empty() {
            return "".to_string();
        }
        let base64_string = general_purpose::STANDARD.encode(image_data);
        format!("data:image/png;base64,{}", base64_string)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenAccount {
    pub token_account: String,
    pub mint: String,
    pub balance: f64,
    pub is_nft: bool,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub uri: Option<String>,
    pub image: Option<String>,
}
