-- alarm: enforce status lifecycle values at the DB level.
-- The application only ever writes 'active' / 'acknowledged' / 'cleared'
-- (see api/alarm_handlers.rs ack/clear and rule_engine inserts), so adding
-- this CHECK on existing data is safe.
ALTER TABLE alarm ADD CONSTRAINT alarm_status_check CHECK (status IN ('active', 'acknowledged', 'cleared'));
