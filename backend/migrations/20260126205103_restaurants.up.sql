-- Restaurants
create table restaurants (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    image_url text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
