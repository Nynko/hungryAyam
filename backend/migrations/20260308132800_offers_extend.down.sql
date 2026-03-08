-- Reverse the additions made by the up migration

-- Drop the new indexes
DROP INDEX IF EXISTS idx_offers_menu;

-- Remove added columns from offers
ALTER TABLE offers DROP COLUMN IF EXISTS menu_id;
ALTER TABLE offers DROP COLUMN IF EXISTS description;
ALTER TABLE offers DROP COLUMN IF EXISTS is_active;