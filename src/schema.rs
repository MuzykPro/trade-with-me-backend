// @generated automatically by Diesel CLI.

diesel::table! {
    metadata (mint_address) {
        mint_address -> Text,
        name -> Nullable<Text>,
        symbol -> Nullable<Text>,
        uri -> Nullable<Text>,
        image -> Nullable<Bytea>,
    }
}
