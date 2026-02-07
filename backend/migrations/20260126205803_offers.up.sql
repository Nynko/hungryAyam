-- Add up migration script here
-- Offers
create table offers (
    id uuid primary key default gen_random_uuid(),
    restaurant_id uuid not null references restaurants(id) on delete cascade,
    title text not null,
    fixed_price_cents integer not null check (fixed_price_cents >= 0),
    created_at timestamptz not null default now(),
    created_by uuid references users(id)
);

-- Offer slots
create table offer_slots (
    id uuid primary key default gen_random_uuid(),
    offer_id uuid not null references offers(id) on delete cascade,
    label text not null,
    min_items integer not null check (min_items >= 0),
    max_items integer not null check (max_items >= min_items)
);

-- Slot constraints (item-only for now)
create table offer_slot_constraints (
    id uuid primary key default gen_random_uuid(),
    slot_id uuid not null references offer_slots(id) on delete cascade,
    allowed_item_id uuid not null references items(id)
);

-- Indexes
create index idx_offers_restaurant on offers(restaurant_id);
create index idx_offer_slots_offer on offer_slots(offer_id);
create index idx_offer_slot_constraints_slot on offer_slot_constraints(slot_id);
