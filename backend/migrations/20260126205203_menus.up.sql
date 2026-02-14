-- Menus - belong to a restaurant, can have multiple per restaurant
create table menus (
    id uuid primary key default gen_random_uuid(),
    restaurant_id uuid not null references restaurants(id) on delete cascade,
    name text not null,
    description text,
    is_active boolean not null default false,
    permanent boolean not null default false,
    created_at timestamptz not null default now(),
    created_by uuid not null references users(id),
    updated_at timestamptz not null default now(),
    updated_by uuid not null references users(id)
);

create index idx_menus_restaurant on menus(restaurant_id);