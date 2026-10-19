-- SMS message template moves from the global app_settings to each restaurant.
ALTER TABLE restaurants ADD COLUMN sms_message_template TEXT;
UPDATE restaurants
SET sms_message_template = (SELECT sms_message_template FROM app_settings WHERE id = 1);
ALTER TABLE app_settings DROP COLUMN sms_message_template;

-- Rendered SMS body, so the OVH fallback sends exactly what the Shortcut would.
ALTER TABLE notification_events ADD COLUMN message TEXT;
-- OVH fallback bookkeeping (retries on transient errors).
ALTER TABLE notification_events ADD COLUMN fallback_attempts INT NOT NULL DEFAULT 0;
ALTER TABLE notification_events ADD COLUMN fallback_error TEXT;
ALTER TABLE notification_events ADD COLUMN fallback_attempted_at TIMESTAMPTZ;
