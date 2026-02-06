
-- Extensions
create extension if not exists "pgcrypto";

-- App settings
create table app_settings (
    id smallint primary key check (id = 1),
    title text not null,
    image_url text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Restaurants
create table restaurants (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    image_url text,
    created_at timestamptz not null default now()
);

-- Menus
create table menus (
    id uuid primary key default gen_random_uuid(),
    restaurant_id uuid not null references restaurants(id) on delete cascade,
    title text not null,
    permanent boolean not null default false,
    created_at timestamptz not null default now()
);

-- Menu sections (supports nesting)
create table menu_sections (
    id uuid primary key default gen_random_uuid(),
    menu_id uuid not null references menus(id) on delete cascade,
    parent_id uuid references menu_sections(id) on delete cascade,
    label text not null,
    position integer not null default 0
);

-- Items (products)
create table items (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    base_price_cents integer not null check (base_price_cents >= 0),
    image_url text,
    active boolean not null default true,
    created_at timestamptz not null default now()
);

-- Items in menu sections
create table menu_section_items (
    section_id uuid not null references menu_sections(id) on delete cascade,
    item_id uuid not null references items(id),
    position integer not null default 0,
    primary key (section_id, item_id)
);

-- Orders
create table orders (
    id uuid primary key default gen_random_uuid(),
    restaurant_id uuid not null references restaurants(id) on delete cascade,
    menu_id uuid not null references menus(id),
    deadline timestamptz,
    allow_late boolean not null default false,
    active boolean not null default true,
    created_at timestamptz not null default now()
);

-- Order users
create table orders_users (
    id uuid primary key default gen_random_uuid(),
    order_id uuid not null references orders(id) on delete cascade,
    user_name text not null,
    user_cookie text not null
);

-- User order items
create table user_order_items (
    id uuid primary key default gen_random_uuid(),
    order_user_id uuid not null references orders_users(id) on delete cascade,
    item_id uuid not null references items(id),
    notes text
);

-- Indexes
create index idx_menus_restaurant on menus(restaurant_id);
create index idx_menu_sections_menu on menu_sections(menu_id);
create index idx_menu_section_items_section on menu_section_items(section_id);
create index idx_orders_restaurant on orders(restaurant_id);
create index idx_orders_active on orders(active);
create index idx_orders_users_order on orders_users(order_id);
create index idx_user_order_items_user on user_order_items(order_user_id);
