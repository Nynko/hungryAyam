-- Restaurant order settings: per-restaurant configuration for order sessions
CREATE TABLE restaurant_order_settings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    restaurant_id uuid NOT NULL UNIQUE REFERENCES restaurants(id) ON DELETE CASCADE,
    default_start_time time NOT NULL DEFAULT '08:00:00',
    default_end_time time NOT NULL DEFAULT '11:00:00',
    sending_method smallint NOT NULL DEFAULT 0, -- 0=Manual, 1=Sms, 2=WhatsApp, 3=Email
    auto_create_session boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_restaurant_order_settings_restaurant ON restaurant_order_settings(restaurant_id);