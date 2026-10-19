ALTER TABLE notification_events DROP COLUMN fallback_attempted_at;
ALTER TABLE notification_events DROP COLUMN fallback_error;
ALTER TABLE notification_events DROP COLUMN fallback_attempts;
ALTER TABLE notification_events DROP COLUMN message;

ALTER TABLE app_settings ADD COLUMN sms_message_template TEXT;
UPDATE app_settings
SET sms_message_template = (
    SELECT sms_message_template FROM restaurants
    WHERE sms_message_template IS NOT NULL
    ORDER BY updated_at DESC LIMIT 1
)
WHERE id = 1;
ALTER TABLE restaurants DROP COLUMN sms_message_template;
