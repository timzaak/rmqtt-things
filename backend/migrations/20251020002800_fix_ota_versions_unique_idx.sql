-- Fix unique index: include version column and exclude soft-deleted rows
DROP INDEX IF EXISTS ota_versions_product_id_key_idx;
CREATE UNIQUE INDEX ota_versions_product_id_key_version_idx ON ota_versions (product_id, key, version) WHERE status = 0;
