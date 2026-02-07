create table user_order_items (
    id uuid primary key default gen_random_uuid(),
    order_user_id uuid not null references orders_users(id) on delete cascade,
    item_id uuid not null references items(id),
    notes text
);

create index idx_user_order_items_user on user_order_items(order_user_id);