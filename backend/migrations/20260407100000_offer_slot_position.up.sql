-- Add position column to offer_slots to allow explicit ordering
ALTER TABLE offer_slots
ADD COLUMN position integer NOT NULL DEFAULT 0;

-- Assign each existing slot a position based on insertion order within its offer
WITH ranked AS (
    SELECT id, ROW_NUMBER() OVER (PARTITION BY offer_id ORDER BY id) - 1 AS pos
    FROM offer_slots
)
UPDATE offer_slots
SET position = ranked.pos
FROM ranked
WHERE offer_slots.id = ranked.id;
