-- Extend offers table with menu_id, description, and is_active columns
--
-- ## Purpose of menu_id
--
-- `menu_id` is an **optional organizational link** between an offer and a menu.
-- It is purely a UI/workflow hint — it does NOT affect constraint resolution.
--
-- What menu_id signals:
--   1. **Admin UX**: "This offer's configuration should be shown inside/alongside
--      this menu's editor." When editing the linked menu, the admin sees the
--      offer fields inline.
--   2. **Customer UX**: When menu_id is set AND the menu is non-permanent,
--      the menu is NOT displayed as a standalone menu to customers. Instead it
--      is presented as a single unified offer card (e.g. "Menu du Jour — €12.50").
--   3. **Operational coupling**: Resetting a non-permanent menu (setting all its
--      items to is_available=false) naturally affects any offer whose slot
--      constraints reference that menu's sections — but this happens implicitly
--      through the Section constraint type, NOT through menu_id itself.
--
-- What menu_id does NOT do:
--   - It does NOT restrict which sections/items/tags a slot constraint can
--     reference. Constraints can freely point at sections from ANY menu
--     (e.g. an offer linked to "Menu du Jour" can have a "Drink" slot that
--     references sections from the permanent "Drinks" menu).
--   - It does NOT drive any reset or activation logic on the backend.
--
-- An offer can only be linked to ONE menu (singular FK). For hybrid offers
-- (e.g. daily specials + permanent drinks), the offer is linked to the
-- primary/rotating menu, and cross-menu references are handled via
-- Section/Tag/Item constraints on individual slots.

ALTER TABLE offers
ADD COLUMN menu_id UUID REFERENCES menus(id) ON DELETE SET NULL,
ADD COLUMN description TEXT,
ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true;

-- Index for menu_id lookups
CREATE INDEX idx_offers_menu ON offers(menu_id);

-- Index for filtering active offers per restaurant
CREATE INDEX idx_offers_restaurant_active ON offers(restaurant_id, is_active);