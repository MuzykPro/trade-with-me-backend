use std::{collections::HashMap, sync::Mutex, time::Duration};

use lru_time_cache::LruCache;
use rust_decimal::Decimal;

pub struct TokenAmountCache {
    cache: Mutex<LruCache::<String, HashMap<String, Decimal>>>
}

impl TokenAmountCache {
    pub fn init() -> Self {
        TokenAmountCache {
            cache: Mutex::new(LruCache::<String, HashMap<String, Decimal>>::with_expiry_duration(Duration::from_secs(600)))
        }
    }

    pub fn get_token_amounts(&self, user_address: &String) -> Option<HashMap<String, Decimal>> {
        self.cache.lock().unwrap().get(user_address).cloned()
    }

    pub fn insert_token_amounts(&self, user_address: String, token_amounts: HashMap<String, Decimal>) {
        self.cache.lock().unwrap().insert(user_address, token_amounts);
    }

}