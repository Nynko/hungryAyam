create extension if not exists "pgcrypto";

create table app_settings (
    id smallint primary key check (id = 1),
    max_menu_nesting_depth smallint not null default 2 check (max_menu_nesting_depth >= 1 and max_menu_nesting_depth <= 10),
    access_hash text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);