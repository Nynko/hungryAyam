-- Items in menu sections
create table menu_section_items (
    section_id uuid not null references menu_sections(id) on delete cascade,
    item_id uuid not null references items(id),
    position integer not null default 0,
    created_at timestamptz not null default now(),
    primary key (section_id, item_id)
);

create index idx_menu_section_items_section on menu_section_items(section_id);
