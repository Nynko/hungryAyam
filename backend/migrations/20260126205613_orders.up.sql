-- Orders
create table orders (
    id uuid primary key default gen_random_uuid(),
    restaurant_id uuid not null references restaurants(id) on delete cascade,
    menu_id uuid not null references menus(id),
    user uuid not null references users(id),
    deadline timestamptz,
    allow_late boolean not null default false,
    active boolean not null default true,
    created_at timestamptz not null default now()
);

create index idx_orders_restaurant on orders(restaurant_id);
create index idx_orders_active on orders(active);
