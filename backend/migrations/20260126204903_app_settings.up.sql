create extension if not exists "pgcrypto";

create table app_settings (
    id smallint primary key check (id = 1),
    title text not null,
    image_url text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
