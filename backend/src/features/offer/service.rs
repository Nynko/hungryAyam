use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::features::offer::{
    domain::{CreateOffer, Offer, UpdateOffer},
    repository::OfferRepository,
};

#[derive(Clone)]
pub struct OfferService {
    repository: OfferRepository,
}

impl OfferService {
    pub fn new(repository: OfferRepository) -> Self {
        Self { repository }
    }

    // ==================== OFFER CRUD ====================

    /// Create a new offer with nested slots and constraints.
    ///
    /// Validations:
    /// - At least one slot is required.
    /// - Each slot must have `min_items <= max_items`.
    /// - Each slot must have at least one constraint.
    /// - Supplement values must be non-negative.
    pub async fn create_offer(&self, request: CreateOffer, user_id: Uuid) -> Result<Offer> {
        self.validate_create_slots(&request)?;
        self.repository.create(request, user_id).await
    }

    /// Get an offer by ID (with slots and constraints).
    pub async fn get_offer(&self, id: Uuid) -> Result<Option<Offer>> {
        self.repository.get_by_id(id).await
    }

    /// List all offers for a restaurant (with slots and constraints).
    pub async fn list_offers_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Offer>> {
        self.repository.get_by_restaurant(restaurant_id).await
    }

    /// List only active offers for a restaurant (with slots and constraints).
    pub async fn list_active_offers_by_restaurant(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Vec<Offer>> {
        self.repository.get_active_by_restaurant(restaurant_id).await
    }

    /// Update an offer. Top-level fields are optional (COALESCE).
    /// If `slots` is provided (`Some`), the entire set of slots is replaced.
    ///
    /// When replacing slots, validates the new slot definitions.
    pub async fn update_offer(&self, request: UpdateOffer, user_id: Uuid) -> Result<Option<Offer>> {
        // If new slots are provided, validate them before persisting.
        if let Some(ref new_slots) = request.slots {
            if new_slots.is_empty() {
                return Err(anyhow!("An offer must have at least one slot"));
            }
            for (i, slot) in new_slots.iter().enumerate() {
                let label = slot
                    .label
                    .as_ref()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| format!("slot {}", i));

                let min = slot.min_items.unwrap_or(0);
                let max = slot.max_items.unwrap_or(0);
                if min < 0 {
                    return Err(anyhow!(
                        "Slot '{}': min_items cannot be negative",
                        label
                    ));
                }
                if max < min {
                    return Err(anyhow!(
                        "Slot '{}': max_items ({}) must be >= min_items ({})",
                        label,
                        max,
                        min
                    ));
                }
                // When replacing, constraints must be provided and non-empty
                if slot
                    .constraints
                    .as_ref()
                    .map_or(true, |c| c.is_empty())
                {
                    return Err(anyhow!(
                        "Slot '{}': at least one constraint is required when replacing slots",
                        label
                    ));
                }
                // Validate supplement_cents if provided
                if let Some(supplement) = slot.supplement_cents {
                    if supplement < 0 {
                        return Err(anyhow!(
                            "Slot '{}': supplement_cents cannot be negative",
                            label
                        ));
                    }
                }
                // Validate constraint supplement_cents
                if let Some(ref constraints) = slot.constraints {
                    for (j, c) in constraints.iter().enumerate() {
                        if let Some(supplement) = c.supplement_cents {
                            if supplement < 0 {
                                return Err(anyhow!(
                                    "Slot '{}', constraint {}: supplement_cents cannot be negative",
                                    label,
                                    j
                                ));
                            }
                        }
                    }
                }
            }
        }

        self.repository.update(request, user_id).await
    }

    /// Delete an offer by ID. Returns true if it existed and was deleted.
    pub async fn delete_offer(&self, id: Uuid) -> Result<bool> {
        self.repository.delete(id).await
    }

    /// Activate an offer (set `is_active = true`).
    pub async fn activate_offer(&self, id: Uuid) -> Result<Option<Offer>> {
        self.repository.set_active(id, true).await
    }

    /// Deactivate an offer (set `is_active = false`).
    pub async fn deactivate_offer(&self, id: Uuid) -> Result<Option<Offer>> {
        self.repository.set_active(id, false).await
    }

    // ==================== ORDER VALIDATION ====================

    /// Validate a user's offer-based order selections.
    ///
    /// This is the main entry-point used by the order service when an order
    /// references an `offer_id`. It checks:
    ///
    /// 1. The offer exists, is active, and belongs to the restaurant.
    /// 2. Every order item has a `slot_id`.
    /// 3. Each slot receives between `min_items` and `max_items` items.
    /// 4. Each item satisfies at least one constraint on its slot.
    ///
    /// Returns the offer (for price lookup) on success.
    pub async fn validate_offer_order(
        &self,
        offer_id: Uuid,
        restaurant_id: Uuid,
        items: &[(Uuid, Option<Uuid>)], // (item_id, slot_id)
    ) -> Result<Offer> {
        let offer = self
            .repository
            .get_by_id(offer_id)
            .await?
            .ok_or_else(|| anyhow!("Offer '{}' not found", offer_id))?;

        if !offer.is_active {
            return Err(anyhow!("Offer '{}' is not currently active", offer.title));
        }

        if offer.restaurant_id != restaurant_id {
            return Err(anyhow!(
                "Offer '{}' does not belong to restaurant '{}'",
                offer_id,
                restaurant_id
            ));
        }

        // Check that every item has a slot_id
        for (item_id, slot_id) in items {
            if slot_id.is_none() {
                return Err(anyhow!(
                    "Item '{}' must have a slot_id when ordering from an offer",
                    item_id
                ));
            }
        }

        // Build a map of slot_id -> list of item_ids
        let mut slot_items: std::collections::HashMap<Uuid, Vec<Uuid>> =
            std::collections::HashMap::new();
        for (item_id, slot_id) in items {
            slot_items
                .entry(slot_id.unwrap())
                .or_default()
                .push(*item_id);
        }

        // Build a set of valid slot IDs from the offer
        let valid_slot_ids: std::collections::HashSet<Uuid> =
            offer.slots.iter().map(|s| s.id).collect();

        // Check that all referenced slot IDs actually belong to this offer
        for sid in slot_items.keys() {
            if !valid_slot_ids.contains(sid) {
                return Err(anyhow!(
                    "Slot '{}' does not belong to offer '{}'",
                    sid,
                    offer_id
                ));
            }
        }

        // Validate each slot's item count and constraint satisfaction
        for slot in &offer.slots {
            let items_for_slot = slot_items.get(&slot.id);
            let count = items_for_slot.map_or(0, |v| v.len()) as i32;

            if count < slot.min_items {
                return Err(anyhow!(
                    "Slot '{}': requires at least {} item(s), but got {}",
                    slot.label,
                    slot.min_items,
                    count
                ));
            }

            if count > slot.max_items {
                return Err(anyhow!(
                    "Slot '{}': allows at most {} item(s), but got {}",
                    slot.label,
                    slot.max_items,
                    count
                ));
            }

            // Validate that each item satisfies at least one constraint
            if let Some(item_ids) = items_for_slot {
                let valid = self
                    .repository
                    .validate_items_for_slot(slot.id, item_ids)
                    .await?;
                if !valid {
                    return Err(anyhow!(
                        "Slot '{}': one or more items are not allowed by the slot constraints",
                        slot.label
                    ));
                }
            }
        }

        Ok(offer)
    }

    /// Compute the total price for an offer-based order.
    ///
    /// The pricing formula is:
    ///
    /// ```text
    /// total = offer.base_price_cents
    ///       + Σ slot.supplement_cents   (for each slot the customer used, i.e. selected ≥ 1 item)
    ///       + Σ constraint.supplement_cents (for each selected item, via its best-matching constraint)
    /// ```
    ///
    /// When an item matches multiple constraints on the same slot, the constraint
    /// with the **lowest** supplement is used (most favorable to the customer).
    ///
    /// This method should be called *after* `validate_offer_order` succeeds.
    pub async fn compute_offer_price(
        &self,
        offer: &Offer,
        items: &[(Uuid, Option<Uuid>)], // (item_id, slot_id)
    ) -> Result<i32> {
        let mut total = *offer.base_price_cents;

        // Build a map of slot_id -> list of item_ids
        let mut slot_items: std::collections::HashMap<Uuid, Vec<Uuid>> =
            std::collections::HashMap::new();
        for (item_id, slot_id) in items {
            if let Some(sid) = slot_id {
                slot_items.entry(*sid).or_default().push(*item_id);
            }
        }

        for slot in &offer.slots {
            let items_for_slot = slot_items.get(&slot.id);
            let count = items_for_slot.map_or(0, |v| v.len());

            if count > 0 {
                // Add slot-level supplement (flat, once per slot used)
                total += slot.supplement_cents;

                // Add constraint-level supplements for each item
                let item_ids = items_for_slot.unwrap();
                let constraint_supplements = self
                    .repository
                    .get_constraint_supplements_for_items(slot.id, item_ids)
                    .await?;

                for item_id in item_ids {
                    if let Some(&supplement) = constraint_supplements.get(item_id) {
                        total += supplement;
                    }
                    // If an item has no matched constraint supplement entry, it means
                    // the constraint supplement is 0 (or the item wasn't matched — but
                    // validation should have caught that already).
                }
            }
        }

        Ok(total)
    }

    /// Get the resolved list of allowed item IDs for a specific slot.
    /// Useful for the frontend to display eligible items.
    pub async fn get_allowed_items_for_slot(&self, slot_id: Uuid) -> Result<Vec<Uuid>> {
        self.repository.get_allowed_item_ids_for_slot(slot_id).await
    }

    // ==================== PRIVATE HELPERS ====================

    /// Validate slot definitions at creation time.
    fn validate_create_slots(&self, request: &CreateOffer) -> Result<()> {
        if request.slots.is_empty() {
            return Err(anyhow!("An offer must have at least one slot"));
        }

        for slot in &request.slots {
            if slot.min_items < 0 {
                return Err(anyhow!(
                    "Slot '{}': min_items cannot be negative",
                    slot.label
                ));
            }
            if slot.max_items < slot.min_items {
                return Err(anyhow!(
                    "Slot '{}': max_items ({}) must be >= min_items ({})",
                    slot.label,
                    slot.max_items,
                    slot.min_items
                ));
            }
            if slot.constraints.is_empty() {
                return Err(anyhow!(
                    "Slot '{}': at least one constraint is required",
                    slot.label
                ));
            }
            if slot.supplement_cents < 0 {
                return Err(anyhow!(
                    "Slot '{}': supplement_cents cannot be negative",
                    slot.label
                ));
            }
            for (j, constraint) in slot.constraints.iter().enumerate() {
                if constraint.supplement_cents < 0 {
                    return Err(anyhow!(
                        "Slot '{}', constraint {}: supplement_cents cannot be negative",
                        slot.label,
                        j
                    ));
                }
            }
        }

        Ok(())
    }
}