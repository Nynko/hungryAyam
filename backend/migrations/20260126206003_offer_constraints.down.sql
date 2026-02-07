-- Drop indexes
drop index if exists idx_offer_slot_constraints_tag;
drop index if exists idx_offer_slot_constraints_section;

-- Drop check constraint
alter table offer_slot_constraints
drop constraint if exists offer_slot_constraints_one_type;

-- Drop columns
alter table offer_slot_constraints
drop column if exists allowed_section_id,
drop column if exists allowed_tag_id;

-- Restore NOT NULL on allowed_item_id
alter table offer_slot_constraints alter column allowed_item_id set not null;
