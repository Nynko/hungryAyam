-- Revert to plain FK (no delete action)
ALTER TABLE order_items
    DROP CONSTRAINT order_items_slot_id_fkey,
    ADD CONSTRAINT order_items_slot_id_fkey
        FOREIGN KEY (slot_id) REFERENCES offer_slots(id);
