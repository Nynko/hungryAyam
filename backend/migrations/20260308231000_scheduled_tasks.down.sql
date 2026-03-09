-- Remove the scheduled_task_log table
DROP TABLE IF EXISTS scheduled_task_log;

-- Remove the new columns from restaurant_order_settings
ALTER TABLE restaurant_order_settings
    DROP COLUMN IF EXISTS menu_reset_time,
    DROP COLUMN IF EXISTS auto_close_session;