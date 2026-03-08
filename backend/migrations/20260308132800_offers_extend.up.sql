-- Extend offers table with menu_id, description, and is_active columns

ALTER TABLE offers
ADD COLUMN menu_id UUID REFERENCES menus(id) ON DELETE SET NULL,
ADD COLUMN description TEXT,
ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true;

-- Index for menu_id lookups
CREATE INDEX idx_offers_menu ON offers(menu_id);

-- Index for filtering active offers per restaurant
CREATE INDEX idx_offers_restaurant_active ON offers(restaurant_id, is_active);