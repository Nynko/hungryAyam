ALTER TABLE restaurant_order_settings
  DROP COLUMN IF EXISTS notify_on_session_close;

ALTER TABLE app_settings
  DROP COLUMN IF EXISTS notification_email;
