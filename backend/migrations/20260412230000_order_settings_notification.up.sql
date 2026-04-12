ALTER TABLE restaurant_order_settings
  ADD COLUMN notify_on_session_close boolean NOT NULL DEFAULT false;

ALTER TABLE app_settings
  ADD COLUMN notification_email text;
