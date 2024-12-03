-- Your SQL goes here
CREATE TABLE Trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    initiator TEXT NOT NULL,           
    counterparty TEXT,          
    status TEXT NOT NULL,              
    status_details JSONB,                         
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP, 
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Trigger to update `updated_at` on row modification
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
   NEW.updated_at = CURRENT_TIMESTAMP;
   RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_updated_at
BEFORE UPDATE ON Trades
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();