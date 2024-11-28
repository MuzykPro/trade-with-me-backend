use std::{cell::RefCell, rc::Rc, sync::Arc};

use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::Mutex;

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

        let token_accounts = self.rpc_client
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
                        let metadata = self.metadata_cache.get_token_metadata(&mint).await;
                        balances.push(TokenAccount {
                            token_account: keyed_account.pubkey.to_string(),
                            mint,
                            balance,
                            is_nft,
                            symbol: metadata
                                .as_ref()
                                .map(|m| {
                                    m.symbol.clone().trim_end_matches(char::from(0)).to_string()
                                })
                                .ok(),
                            name: metadata
                                .as_ref()
                                .map(|m| m.name.clone().trim_end_matches(char::from(0)).to_string())
                                .ok(),
                            uri: metadata
                                .as_ref()
                                .map(|m| m.uri.clone().trim_end_matches(char::from(0)).to_string())
                                .ok(),
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
}
