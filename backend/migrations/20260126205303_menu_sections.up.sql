-- Menu sections (supports nesting via parent_id)
create table menu_sections (
    id uuid primary key default gen_random_uuid(),
    menu_id uuid not null references menus(id) on delete cascade,
    parent_id uuid references menu_sections(id) on delete cascade,
    name text not null,
    description text,
    position integer not null default 0,
    is_active boolean not null default true,
    created_at timestamptz not null default now(),
    created_by uuid not null references users(id),
    updated_at timestamptz not null default now(),
    updated_by uuid not null references users(id)
);

create index idx_menu_sections_menu on menu_sections(menu_id);
create index idx_menu_sections_parent on menu_sections(parent_id);