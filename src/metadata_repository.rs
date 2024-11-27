fn get_all_saved_mint_addresses() -> Vec<String> {}

fn save_metadata() {
    
}
struct MetadataEntity {
    pub mint_address: String,
    pub is_nft: bool,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub image: Vec<u8>,
}
