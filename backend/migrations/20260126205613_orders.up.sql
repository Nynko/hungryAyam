-- Orders
create table orders (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users(id),
    session_id uuid not null references order_sessions(id) on delete cascade,
    offer_id uuid references offers(id),
    total_price_cents integer not null,
    created_at timestamptz not null default now()
);

create index idx_orders_session on orders(session_id);
create index idx_orders_user on orders(user_id);
