-- Add up migration script here
create table tags (
    id uuid primary key default gen_random_uuid(),
    name text not null unique
);

create table item_tags (
    item_id uuid not null references items(id) on delete cascade,
    tag_id uuid not null references tags(id) on delete cascade,
    primary key (item_id, tag_id)
);

create index idx_item_tags_item on item_tags(item_id);
create index idx_item_tags_tag on item_tags(tag_id);
