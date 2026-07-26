-- action_invocation table (thing-model-extension feature, design §4.3.2).
-- Pure additive: 1 new table + 3 indexes, zero changes to existing tables.
-- Physically isolated from property_command per design A2 (user decision in §4.1):
-- one-shot actions (reboot / unlock / buzzer) live here, distinct from one-shot
-- property writes. Both tables mirror the same status semantics (CommandStatus).
-- No foreign keys — matches the existing property_command convention.

CREATE TABLE IF NOT EXISTS action_invocation (
    id           BIGSERIAL    PRIMARY KEY,
    product_id   TEXT         NOT NULL,
    device_id    TEXT         NOT NULL,
    service_type TEXT         NOT NULL,                          -- e.g. reboot; free-form identifier (design A4)
    params       JSONB        NOT NULL DEFAULT '{}'::jsonb,      -- action input parameters
    status       INT2         NOT NULL DEFAULT 0,                -- 0 pending / 1 sent / 2 success / 3 failed / 4 deleted (mirrors CommandStatus)
    created_time TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_time TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

COMMENT ON COLUMN action_invocation.status IS '0: pending, 1: sent, 2: success, 3: failed, 4: deleted';

-- Supports the atomic claim lookup (product, device, status = Pending).
CREATE INDEX IF NOT EXISTS idx_action_invocation_product_device_status
    ON action_invocation (product_id, device_id, status);

-- Supports the admin history query (ORDER BY updated_time DESC) and the
-- created_time/id ordering fallback used by the drain loop.
CREATE INDEX IF NOT EXISTS idx_action_invocation_product_device_updated_time
    ON action_invocation (product_id, device_id, updated_time DESC);

-- Aligns with the existing property_command pattern; supports time-based scans.
CREATE INDEX IF NOT EXISTS idx_action_invocation_created_time
    ON action_invocation (created_time);
