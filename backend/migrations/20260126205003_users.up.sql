create table users (
    id uuid primary key default gen_random_uuid(),
    name text,
    email text unique, -- nullable for unauthenticated users
    auth_method text,  -- nullable or enum (e.g., 'password', 'oauth', etc.)
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Optionally, add indexes for faster lookups/statistics
create index idx_users_email on users(email);
create index idx_users_auth_method on users(auth_method);