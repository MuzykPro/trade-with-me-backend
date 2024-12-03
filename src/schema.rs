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

diesel::table! {
    trades (id) {
        id -> Uuid,
        initiator -> Text,
        counterparty -> Nullable<Text>,
        status -> Text,
        status_details -> Nullable<Jsonb>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    metadata,
    trades,
);
