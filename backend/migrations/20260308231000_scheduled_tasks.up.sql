-- Add menu reset time and auto-close session settings to restaurant_order_settings
ALTER TABLE restaurant_order_settings
    ADD COLUMN menu_reset_time time DEFAULT NULL,
    ADD COLUMN auto_close_session boolean NOT NULL DEFAULT true;

-- Scheduled task log: tracks the last time a periodic task ran for a given entity.
-- Used to prevent double-execution (e.g. resetting the same menu twice in one day).
CREATE TABLE scheduled_task_log (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    restaurant_id uuid NOT NULL REFERENCES restaurants(id) ON DELETE CASCADE,
    -- Task kind: e.g. 'menu_reset', 'session_auto_close'
    task_kind text NOT NULL,
    -- The date (in restaurant-local timezone) on which this task last executed.
    -- For daily tasks like menu_reset, storing the date prevents re-runs on the same day.
    last_executed_date date NOT NULL,
    last_executed_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    -- One log entry per restaurant per task kind per date
    CONSTRAINT uq_task_log_restaurant_kind_date UNIQUE (restaurant_id, task_kind, last_executed_date)
);

CREATE INDEX idx_scheduled_task_log_restaurant ON scheduled_task_log(restaurant_id);
CREATE INDEX idx_scheduled_task_log_kind ON scheduled_task_log(task_kind);

-- Add a comment explaining the new columns
COMMENT ON COLUMN restaurant_order_settings.menu_reset_time IS
    'Time of day (in the restaurant''s timezone) when non-permanent menus should be automatically reset '
    '(all items set to is_available = false). NULL means no automatic reset.';

COMMENT ON COLUMN restaurant_order_settings.auto_close_session IS
    'When true, order sessions are automatically closed when their end_date passes. '
    'Defaults to true. The scheduler checks every minute and transitions Open sessions '
    'whose end_date is in the past to Closed status.';