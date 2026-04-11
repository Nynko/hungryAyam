-- Fix FK so that deleting a slot (e.g. during offer update) nulls out the
-- reference in order_items rather than blocking the delete.
ALTER TABLE order_items
    DROP CONSTRAINT order_items_slot_id_fkey,
    ADD CONSTRAINT order_items_slot_id_fkey
        FOREIGN KEY (slot_id) REFERENCES offer_slots(id) ON DELETE SET NULL;
