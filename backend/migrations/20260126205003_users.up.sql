create table users (
    id uuid primary key default gen_random_uuid(),
    name text not null unique,
    email text unique, -- nullable for guest (NameWithCookie) users
    auth_method text not null,  -- 'NameWithCookie' or 'Password'
    password_hash text, -- argon2 hash, only for Password users
    role text, -- 'Viewer', 'User', 'Admin' — only for Password users, NULL for guests
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Indexes for faster lookups
create index idx_users_email on users(email);
create index idx_users_auth_method on users(auth_method);