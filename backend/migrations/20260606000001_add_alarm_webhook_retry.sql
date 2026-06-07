-- alarm: add trigger_type, webhook retry fields, and retry index

-- trigger_type is needed by the retry task (BE-D08) to reconstruct webhook payloads.
-- Default '' for existing rows; new rows should always set it explicitly.
ALTER TABLE alarm ADD COLUMN IF NOT EXISTS trigger_type TEXT NOT NULL DEFAULT '';

-- Webhook retry state machine fields
ALTER TABLE alarm ADD COLUMN IF NOT EXISTS webhook_retries_left INT2 NOT NULL DEFAULT 0;
ALTER TABLE alarm ADD COLUMN IF NOT EXISTS webhook_next_retry_at TIMESTAMPTZ DEFAULT NULL;

COMMENT ON COLUMN alarm.trigger_type IS 'property / event / device_online / device_offline — copied from the matched alarm_rule at creation time';
COMMENT ON COLUMN alarm.webhook_retries_left IS 'remaining retry attempts, 0 = no pending retry';
COMMENT ON COLUMN alarm.webhook_next_retry_at IS 'next scheduled retry time, NULL = no retry scheduled';

-- Partial index for the retry background task: only rows with a pending retry.
CREATE INDEX IF NOT EXISTS idx_alarm_webhook_retry
    ON alarm (webhook_next_retry_at)
    WHERE webhook_status = 1 AND webhook_retries_left > 0;
