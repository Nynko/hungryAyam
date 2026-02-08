-- Items (products) - belong to a restaurant
create table items (
    id uuid primary key default gen_random_uuid(),
    restaurant_id uuid not null references restaurants(id) on delete cascade,
    name text not null,
    description text,
    base_price_cents integer not null check (base_price_cents >= 0),
    image_url text,
    active boolean not null default true,
    created_at timestamptz not null default now(),
    created_by uuid not null references users(id),
    updated_at timestamptz not null default now(),
    updated_by uuid not null references users(id)
);

create index idx_items_restaurant on items(restaurant_id);
