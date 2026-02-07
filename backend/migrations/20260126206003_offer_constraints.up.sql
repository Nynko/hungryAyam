-- Add up migration script here

alter table offer_slot_constraints alter column allowed_item_id drop not null;

alter table offer_slot_constraints
add column allowed_tag_id uuid references tags(id),
add column allowed_section_id uuid references menu_sections(id);


alter table offer_slot_constraints
add constraint offer_slot_constraints_one_type
check (
    (allowed_item_id is not null)::int +
    (allowed_tag_id is not null)::int +
    (allowed_section_id is not null)::int = 1
);

create index idx_offer_slot_constraints_tag
on offer_slot_constraints(allowed_tag_id);

create index idx_offer_slot_constraints_section
on offer_slot_constraints(allowed_section_id);
