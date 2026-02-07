-- Menu sections (supports nesting)
create table menu_sections (
    id uuid primary key default gen_random_uuid(),
    menu_id uuid not null references menus(id) on delete cascade,
    parent_id uuid references menu_sections(id) on delete cascade,
    label text not null,
    position integer not null default 0,
    created_at timestamptz not null default now(),
);

create index idx_menu_sections_menu on menu_sections(menu_id);
