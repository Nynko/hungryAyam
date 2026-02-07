-- Items (products)
create table items (
    id uuid primary key default gen_random_uuid(),
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
