-- Factory metadata layer (support-multiple-device feature, design §4.3.2).
-- Pure additive: 4 new tables + 2 indexes, zero changes to existing tables.
-- All four tables intentionally have NO foreign keys — device/component rows and
-- associations may arrive in any order (out-of-order normal, design R3) and the
-- metadata layer must accept each independently.

-- Device-level factory metadata (reserved this round; no write entry point yet).
-- Table exists so the GET /api/admin/factory/devices/{deviceSn} response schema
-- has a stable extension slot for deviceMetadata.
CREATE TABLE IF NOT EXISTS factory_device_metadata (
    device_sn        TEXT        NOT NULL,
    metadata         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    file_attachments JSONB       NOT NULL DEFAULT '[]'::jsonb,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE factory_device_metadata ADD PRIMARY KEY (device_sn);

-- Component-level factory metadata. component_type is free TEXT (default
-- 'camera'); not enum-constrained so radar/sensor extensions need no migration.
CREATE TABLE IF NOT EXISTS factory_component_metadata (
    component_sn     TEXT        NOT NULL,
    component_type   TEXT        NOT NULL DEFAULT 'camera',
    metadata         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    file_attachments JSONB       NOT NULL DEFAULT '[]'::jsonb,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE factory_component_metadata ADD PRIMARY KEY (component_sn);

-- Device ↔ component association. Composite PK prevents duplicate associations.
-- component_type is NULLable here (a hint carried at association time; the
-- metadata table's value takes precedence in the merged view).
CREATE TABLE IF NOT EXISTS factory_component_association (
    device_sn      TEXT        NOT NULL,
    component_sn   TEXT        NOT NULL,
    component_type TEXT,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE factory_component_association
    ADD PRIMARY KEY (device_sn, component_sn);

-- Reverse lookup by device_sn (admin/device queries filter on device_sn).
CREATE INDEX IF NOT EXISTS idx_fca_device_sn
    ON factory_component_association (device_sn);

-- Change log for component metadata overwrites (design R5: same-SN re-report is
-- idempotent overwrite + before/after snapshot log). `before` is NULL on the
-- very first report; `after` always carries the new snapshot.
CREATE TABLE IF NOT EXISTS factory_metadata_change_log (
    id           BIGSERIAL   PRIMARY KEY,
    component_sn TEXT        NOT NULL,
    before       JSONB,
    after        JSONB       NOT NULL,
    actor        TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Time-descending paginated change-log lookup per component_sn (admin API D).
CREATE INDEX IF NOT EXISTS idx_fmcl_component_sn_created_at
    ON factory_metadata_change_log (component_sn, created_at DESC);
