create table order_items (
    id uuid primary key default gen_random_uuid(),
    order_id uuid not null references orders(id) on delete cascade,
    item_id uuid not null references items(id),
    slot_id uuid references offer_slots(id), -- If it is part of an offer slot
    notes text
);


create index idx_order_items_user on order_items(order_id);
