create table user_sessions (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users(id) on delete cascade,
    token text not null unique,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

-- Fast lookup by token (used on every authenticated request)
create index idx_user_sessions_token on user_sessions(token);

-- Fast cleanup of expired sessions
create index idx_user_sessions_expires_at on user_sessions(expires_at);

-- Fast lookup of sessions by user (for logout-all, session listing)
create index idx_user_sessions_user_id on user_sessions(user_id);