-- Rename fixed_price_cents → base_price_cents on offers table
ALTER TABLE offers RENAME COLUMN fixed_price_cents TO base_price_cents;

-- Add supplement_cents to offer_slots (flat surcharge when customer uses this slot)
-- Default 0 = included in base price
ALTER TABLE offer_slots
ADD COLUMN supplement_cents INTEGER NOT NULL DEFAULT 0 CHECK (supplement_cents >= 0);

-- Add supplement_cents to offer_slot_constraints (per-constraint surcharge on top of slot supplement)
-- Default 0 = no extra charge for items matched by this constraint
ALTER TABLE offer_slot_constraints
ADD COLUMN supplement_cents INTEGER NOT NULL DEFAULT 0 CHECK (supplement_cents >= 0);