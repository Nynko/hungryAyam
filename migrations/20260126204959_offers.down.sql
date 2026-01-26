-- Add down migration script here
alter table offer_slot_constraints
drop constraint if exists offer_slot_constraints_one_type;

alter table offer_slot_constraints
drop column if exists allowed_section_id,
drop column if exists allowed_tag_id;

drop table if exists item_tags;
drop table if exists tags;
