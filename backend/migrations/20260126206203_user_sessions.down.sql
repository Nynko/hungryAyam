drop index if exists idx_user_sessions_user_id;
drop index if exists idx_user_sessions_expires_at;
drop index if exists idx_user_sessions_token;
drop table if exists user_sessions;