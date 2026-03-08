-- Remove supplement_cents from offer_slot_constraints
ALTER TABLE offer_slot_constraints
DROP COLUMN supplement_cents;

-- Remove supplement_cents from offer_slots
ALTER TABLE offer_slots
DROP COLUMN supplement_cents;

-- Rename base_price_cents back to fixed_price_cents on offers table
ALTER TABLE offers RENAME COLUMN base_price_cents TO fixed_price_cents;