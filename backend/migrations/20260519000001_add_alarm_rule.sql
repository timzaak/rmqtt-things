-- 告警规则表
CREATE TABLE IF NOT EXISTS alarm_rule (
    id              BIGSERIAL PRIMARY KEY,
    product_id      TEXT        NOT NULL,
    name            TEXT        NOT NULL,
    description     TEXT,
    trigger_type    TEXT        NOT NULL,
    trigger_config  JSONB       NOT NULL DEFAULT '{}',
    condition       JSONB       NOT NULL DEFAULT '{}',
    actions         JSONB       NOT NULL,
    enabled         BOOLEAN     NOT NULL DEFAULT true,
    throttle_minutes INT        NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON COLUMN alarm_rule.trigger_type IS 'property / event / device_online / device_offline';
COMMENT ON COLUMN alarm_rule.throttle_minutes IS 'deduplication window in minutes, 0 = no dedup';

CREATE INDEX IF NOT EXISTS idx_alarm_rule_product_enabled
    ON alarm_rule (product_id, enabled);

-- 告警记录表
CREATE TABLE IF NOT EXISTS alarm (
    id              BIGSERIAL PRIMARY KEY,
    rule_id         BIGINT      NOT NULL,
    rule_name       TEXT        NOT NULL,
    product_id      TEXT        NOT NULL,
    device_id       TEXT        NOT NULL,
    level           INT2        NOT NULL DEFAULT 0,
    message         TEXT,
    trigger_value   JSONB,
    acknowledged    BOOLEAN     NOT NULL DEFAULT false,
    webhook_status  INT2        DEFAULT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON COLUMN alarm.level IS '0=info, 1=warning, 2=critical';
COMMENT ON COLUMN alarm.webhook_status IS 'NULL=not configured, 0=success, 1=failed';

CREATE INDEX IF NOT EXISTS idx_alarm_product_created
    ON alarm (product_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_alarm_device_created
    ON alarm (device_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_alarm_acknowledged
    ON alarm (acknowledged) WHERE NOT acknowledged;
