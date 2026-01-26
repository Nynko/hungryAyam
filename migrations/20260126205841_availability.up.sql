-- Add up migration script here
-- Availability rules
create table availability_rules (
    id uuid primary key default gen_random_uuid(),
    valid_from date,
    valid_to date,
    start_time time,
    end_time time,
    weekdays smallint[],
    active boolean not null default true,
    check (
        valid_from is null
        or valid_to is null
        or valid_from <= valid_to
    )
);

-- Attach availability
alter table menus
add column availability_rule_id uuid references availability_rules(id);

alter table offers
add column availability_rule_id uuid references availability_rules(id);

alter table items
add column availability_rule_id uuid references availability_rules(id);

-- Index
create index idx_availability_active on availability_rules(active);
