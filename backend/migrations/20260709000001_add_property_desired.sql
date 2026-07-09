-- property_desired: persisted desired state per device (symmetric to property_latest).
-- Stores bare desired property values (no {value, time} wrapping). Upsert logic
-- is implemented in the Rust repository (RFC 7396 subset merge: null = delete key);
-- the column is overwritten as a whole document, never via JSONB `||`.
CREATE TABLE IF NOT EXISTS property_desired (
    product_id   TEXT        NOT NULL,
    device_id    TEXT        NOT NULL,
    desired      JSONB       NOT NULL DEFAULT '{}'::jsonb,
    updated_time TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE property_desired ADD PRIMARY KEY (product_id, device_id);
