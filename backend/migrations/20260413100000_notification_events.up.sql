CREATE TABLE notification_events (
    sig                 TEXT PRIMARY KEY,
    session_id          UUID NOT NULL REFERENCES order_sessions(id) ON DELETE CASCADE,
    ts                  BIGINT NOT NULL,
    phone               TEXT NOT NULL,
    time_str            TEXT NOT NULL,
    orders              TEXT NOT NULL,
    restaurant          TEXT NOT NULL,
    email_sent_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    email_received_at   TIMESTAMPTZ,
    sms_sent_at         TIMESTAMPTZ,
    fallback_sent_at    TIMESTAMPTZ
);
