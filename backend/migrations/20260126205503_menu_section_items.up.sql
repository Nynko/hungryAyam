-- Menu section items - links sections to catalog items
create table menu_section_items (
    id uuid primary key default gen_random_uuid(),
    section_id uuid not null references menu_sections(id) on delete cascade,
    item_id uuid not null references items(id),
    position integer not null default 0,
    price_override_cents integer check (price_override_cents is null or price_override_cents >= 0),
    is_available boolean not null default true,
    created_at timestamptz not null default now(),
    created_by uuid not null references users(id),
    updated_at timestamptz not null default now(),
    updated_by uuid not null references users(id),
    unique (section_id, item_id)
);

create index idx_menu_section_items_section on menu_section_items(section_id);
create index idx_menu_section_items_item on menu_section_items(item_id);