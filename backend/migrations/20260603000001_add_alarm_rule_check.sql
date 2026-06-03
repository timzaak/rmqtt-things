-- alarm_rule: add duration condition and clear condition
ALTER TABLE alarm_rule ADD COLUMN duration_minutes INT NOT NULL DEFAULT 0;
ALTER TABLE alarm_rule ADD COLUMN clear_condition JSONB DEFAULT NULL;

COMMENT ON COLUMN alarm_rule.duration_minutes IS 'duration condition in minutes, 0 = instant trigger, only for property trigger type';
COMMENT ON COLUMN alarm_rule.clear_condition IS 'clear condition, same format as condition, only for property trigger type';

-- alarm: add status lifecycle and cleared_at
ALTER TABLE alarm ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
ALTER TABLE alarm ADD COLUMN cleared_at TIMESTAMPTZ DEFAULT NULL;

COMMENT ON COLUMN alarm.status IS 'active / acknowledged / cleared';
COMMENT ON COLUMN alarm.cleared_at IS 'timestamp when alarm was cleared';

-- backfill status from acknowledged
UPDATE alarm SET status = CASE WHEN acknowledged THEN 'acknowledged' ELSE 'active' END;

-- index for clear condition evaluation (high frequency)
CREATE INDEX idx_alarm_rule_device_active ON alarm (rule_id, device_id) WHERE status = 'active';
-- index for admin alarm list filtering non-cleared records (low frequency)
CREATE INDEX idx_alarm_status ON alarm (status) WHERE status != 'cleared';
