-- Your SQL goes here
CREATE TABLE metadata (
    mint_address TEXT PRIMARY KEY,
    name TEXT,
    symbol TEXT,
    uri TEXT,
    image BYTEA
);