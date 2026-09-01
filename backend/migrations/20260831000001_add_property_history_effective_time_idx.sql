-- Chart series/keys-discovery queries filter by (product_id, device_id) equality
-- plus a COALESCE(reported_time, created_time) range. The COALESCE expression
-- cannot use the existing reported_time / created_time indexes, so without this
-- index every chart query degrades to scanning the device's full history.
--
-- The expression must stay semantically identical to EFFECTIVE_TIME_EXPR in
-- src/db/database.rs (the queries are built from that constant); a mismatch
-- silently forfeits this index.
CREATE INDEX IF NOT EXISTS idx_property_history_device_effective_time
ON property_history (product_id, device_id, (COALESCE(reported_time, created_time)) DESC);
