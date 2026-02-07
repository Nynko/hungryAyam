create table orders_users (
    id uuid primary key default gen_random_uuid(),
    order_id uuid not null references orders(id) on delete cascade,
    user_id uuid not null references users(id) on delete cascade,
    user_cookie text, -- for guest users, can be used for session tracking
    created_at timestamptz not null default now()
);

create index idx_orders_users_order on orders_users(order_id);