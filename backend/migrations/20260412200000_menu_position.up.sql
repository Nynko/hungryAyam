-- Add position column to menus to allow explicit ordering within a restaurant
ALTER TABLE menus
ADD COLUMN position integer NOT NULL DEFAULT 0;

-- Assign each existing menu a position based on creation order within its restaurant
WITH ranked AS (
    SELECT id, ROW_NUMBER() OVER (PARTITION BY restaurant_id ORDER BY created_at, id) - 1 AS pos
    FROM menus
)
UPDATE menus
SET position = ranked.pos
FROM ranked
WHERE menus.id = ranked.id;
