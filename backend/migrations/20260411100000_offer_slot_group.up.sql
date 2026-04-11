-- Add slot_group to offer_slots: slots sharing the same group name are treated
-- as a unit for validation, pricing, and display (e.g. "Plat" + "Accompagnement").
ALTER TABLE offer_slots
ADD COLUMN slot_group text NULL;
