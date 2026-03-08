use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::offer::{
        db_model::{OfferRow, OfferSlotConstraintRow, OfferSlotRow},
        domain::{
            CreateOffer, CreateOfferSlot, CreateOfferSlotConstraint, Offer, OfferSlot,
            OfferSlotConstraint, SlotConstraintKind, UpdateOffer,
        },
    },
    types::{name::Name, price::PriceCents},
};

#[derive(Clone)]
pub struct OfferRepository {
    pool: PgPool,
}

impl OfferRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ==================== OFFER CRUD ====================

    /// Create a full offer with nested slots and constraints in a single transaction.
    pub async fn create(&self, request: CreateOffer, user_id: Uuid) -> Result<Offer> {
        let mut tx = self.pool.begin().await?;

        let offer_row = sqlx::query_as!(
            OfferRow,
            r#"
            INSERT INTO offers (restaurant_id, menu_id, title, description, fixed_price_cents, is_active, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                fixed_price_cents as "fixed_price_cents: PriceCents",
                is_active,
                created_at,
                created_by
            "#,
            request.restaurant_id,
            request.menu_id,
            request.title.as_ref(),
            request.description,
            request.fixed_price_cents.as_ref(),
            request.is_active,
            user_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        let mut slots = Vec::with_capacity(request.slots.len());
        for slot_req in &request.slots {
            let slot = self
                .create_slot_in_tx(&mut tx, offer_row.id, slot_req)
                .await?;
            slots.push(slot);
        }

        tx.commit().await?;

        Ok(self.row_to_offer(offer_row, slots))
    }

    /// Get an offer by ID with all slots and constraints loaded.
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Offer>> {
        let offer_row = sqlx::query_as!(
            OfferRow,
            r#"
            SELECT
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                fixed_price_cents as "fixed_price_cents: PriceCents",
                is_active,
                created_at,
                created_by
            FROM offers
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        match offer_row {
            Some(row) => {
                let slots = self.load_slots_for_offer(row.id).await?;
                Ok(Some(self.row_to_offer(row, slots)))
            }
            None => Ok(None),
        }
    }

    /// List all offers for a restaurant (with slots and constraints).
    pub async fn get_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Offer>> {
        let rows = sqlx::query_as!(
            OfferRow,
            r#"
            SELECT
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                fixed_price_cents as "fixed_price_cents: PriceCents",
                is_active,
                created_at,
                created_by
            FROM offers
            WHERE restaurant_id = $1
            ORDER BY created_at DESC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_offers_with_slots(rows).await
    }

    /// List only active offers for a restaurant (with slots and constraints).
    pub async fn get_active_by_restaurant(&self, restaurant_id: Uuid) -> Result<Vec<Offer>> {
        let rows = sqlx::query_as!(
            OfferRow,
            r#"
            SELECT
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                fixed_price_cents as "fixed_price_cents: PriceCents",
                is_active,
                created_at,
                created_by
            FROM offers
            WHERE restaurant_id = $1 AND is_active = true
            ORDER BY created_at DESC
            "#,
            restaurant_id
        )
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_offers_with_slots(rows).await
    }

    /// Update an offer. Top-level fields use COALESCE (all_optional).
    /// If `slots` is provided (`Some`), the entire set of slots is replaced
    /// (existing slots + constraints are deleted and re-created).
    pub async fn update(&self, request: UpdateOffer, _user_id: Uuid) -> Result<Option<Offer>> {
        let mut tx = self.pool.begin().await?;

        let offer_row = sqlx::query_as!(
            OfferRow,
            r#"
            UPDATE offers
            SET
                menu_id      = COALESCE($1, menu_id),
                title        = COALESCE($2, title),
                description  = COALESCE($3, description),
                fixed_price_cents = COALESCE($4, fixed_price_cents),
                is_active    = COALESCE($5, is_active)
            WHERE id = $6
            RETURNING
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                fixed_price_cents as "fixed_price_cents: PriceCents",
                is_active,
                created_at,
                created_by
            "#,
            request.menu_id,
            request.title.as_ref().map(|n| n.as_ref()),
            request.description,
            request.fixed_price_cents.as_ref().map(|p| p.as_ref()),
            request.is_active,
            request.id,
        )
        .fetch_optional(&mut *tx)
        .await?;

        let offer_row = match offer_row {
            Some(r) => r,
            None => return Ok(None),
        };

        // If slots were provided, replace-all
        let slots = if let Some(new_slots) = request.slots {
            // Delete all existing slots (cascades to constraints)
            sqlx::query!("DELETE FROM offer_slots WHERE offer_id = $1", offer_row.id)
                .execute(&mut *tx)
                .await?;

            let mut slots = Vec::with_capacity(new_slots.len());
            for slot_req in &new_slots {
                let slot = self
                    .create_slot_in_tx(&mut tx, offer_row.id, &slot_req.into())
                    .await?;
                slots.push(slot);
            }
            slots
        } else {
            // Load existing slots
            self.load_slots_for_offer_tx(&mut tx, offer_row.id).await?
        };

        tx.commit().await?;

        Ok(Some(self.row_to_offer(offer_row, slots)))
    }

    /// Delete an offer by ID. Returns true if a row was deleted.
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM offers WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Set the `is_active` flag on an offer.
    pub async fn set_active(&self, id: Uuid, active: bool) -> Result<Option<Offer>> {
        let offer_row = sqlx::query_as!(
            OfferRow,
            r#"
            UPDATE offers
            SET is_active = $1
            WHERE id = $2
            RETURNING
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                fixed_price_cents as "fixed_price_cents: PriceCents",
                is_active,
                created_at,
                created_by
            "#,
            active,
            id,
        )
        .fetch_optional(&self.pool)
        .await?;

        match offer_row {
            Some(row) => {
                let slots = self.load_slots_for_offer(row.id).await?;
                Ok(Some(self.row_to_offer(row, slots)))
            }
            None => Ok(None),
        }
    }

    // ==================== VALIDATION QUERIES ====================

    /// Check whether all the given item IDs satisfy at least one constraint
    /// on the specified slot. Returns true only if every item is valid.
    ///
    /// The allowed set is the union of:
    /// - directly allowed item IDs
    /// - items carrying any of the allowed tag IDs
    /// - available items in any of the allowed section IDs
    pub async fn validate_items_for_slot(
        &self,
        slot_id: Uuid,
        item_ids: &[Uuid],
    ) -> Result<bool> {
        if item_ids.is_empty() {
            return Ok(true);
        }

        let unique_ids: Vec<Uuid> = {
            let mut set = std::collections::HashSet::new();
            item_ids
                .iter()
                .filter(|id| set.insert(**id))
                .copied()
                .collect()
        };

        // Count how many of the unique item IDs satisfy at least one constraint.
        let row = sqlx::query!(
            r#"
            WITH allowed_items AS (
                -- Items directly allowed
                SELECT allowed_item_id AS item_id
                FROM offer_slot_constraints
                WHERE slot_id = $1 AND allowed_item_id IS NOT NULL

                UNION

                -- Items matching an allowed tag
                SELECT it.item_id
                FROM offer_slot_constraints osc
                JOIN item_tags it ON it.tag_id = osc.allowed_tag_id
                WHERE osc.slot_id = $1 AND osc.allowed_tag_id IS NOT NULL

                UNION

                -- Available items in an allowed section
                SELECT msi.item_id
                FROM offer_slot_constraints osc
                JOIN menu_section_items msi ON msi.section_id = osc.allowed_section_id
                    AND msi.is_available = true
                WHERE osc.slot_id = $1 AND osc.allowed_section_id IS NOT NULL
            )
            SELECT COUNT(DISTINCT u.id) as "cnt!"
            FROM UNNEST($2::uuid[]) AS u(id)
            WHERE u.id IN (SELECT item_id FROM allowed_items)
            "#,
            slot_id,
            &unique_ids,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.cnt as usize == unique_ids.len())
    }

    /// Get the allowed item IDs for a slot (resolving tags and sections).
    /// Useful for the frontend to know which items to show.
    pub async fn get_allowed_item_ids_for_slot(&self, slot_id: Uuid) -> Result<Vec<Uuid>> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT item_id as "item_id!"
            FROM (
                -- Directly allowed items
                SELECT allowed_item_id AS item_id
                FROM offer_slot_constraints
                WHERE slot_id = $1 AND allowed_item_id IS NOT NULL

                UNION

                -- Items matching an allowed tag
                SELECT it.item_id
                FROM offer_slot_constraints osc
                JOIN item_tags it ON it.tag_id = osc.allowed_tag_id
                WHERE osc.slot_id = $1 AND osc.allowed_tag_id IS NOT NULL

                UNION

                -- Available items in an allowed section
                SELECT msi.item_id
                FROM offer_slot_constraints osc
                JOIN menu_section_items msi ON msi.section_id = osc.allowed_section_id
                    AND msi.is_available = true
                WHERE osc.slot_id = $1 AND osc.allowed_section_id IS NOT NULL
            ) AS combined
            "#,
            slot_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.item_id).collect())
    }

    // ==================== PRIVATE HELPERS ====================

    /// Create a single slot with its constraints inside an existing transaction.
    async fn create_slot_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        offer_id: Uuid,
        request: &CreateOfferSlot,
    ) -> Result<OfferSlot> {
        let slot_row = sqlx::query_as!(
            OfferSlotRow,
            r#"
            INSERT INTO offer_slots (offer_id, label, min_items, max_items)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id,
                offer_id,
                label as "label: Name",
                min_items,
                max_items
            "#,
            offer_id,
            request.label.as_ref(),
            request.min_items,
            request.max_items,
        )
        .fetch_one(&mut **tx)
        .await?;

        let mut constraints = Vec::with_capacity(request.constraints.len());
        for c_req in &request.constraints {
            let constraint = self
                .create_constraint_in_tx(tx, slot_row.id, c_req)
                .await?;
            constraints.push(constraint);
        }

        Ok(self.slot_row_to_domain(slot_row, constraints))
    }

    /// Create a single constraint inside an existing transaction.
    async fn create_constraint_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        slot_id: Uuid,
        request: &CreateOfferSlotConstraint,
    ) -> Result<OfferSlotConstraint> {
        let row = sqlx::query_as!(
            OfferSlotConstraintRow,
            r#"
            INSERT INTO offer_slot_constraints (slot_id, allowed_item_id, allowed_tag_id, allowed_section_id)
            VALUES ($1, $2, $3, $4)
            RETURNING id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id
            "#,
            slot_id,
            request.kind.item_id(),
            request.kind.tag_id(),
            request.kind.section_id(),
        )
        .fetch_one(&mut **tx)
        .await?;

        self.constraint_row_to_domain(row)
    }

    /// Load all slots (with constraints) for a given offer.
    async fn load_slots_for_offer(&self, offer_id: Uuid) -> Result<Vec<OfferSlot>> {
        let slot_rows = sqlx::query_as!(
            OfferSlotRow,
            r#"
            SELECT
                id,
                offer_id,
                label as "label: Name",
                min_items,
                max_items
            FROM offer_slots
            WHERE offer_id = $1
            ORDER BY id
            "#,
            offer_id,
        )
        .fetch_all(&self.pool)
        .await?;

        self.slot_rows_with_constraints(slot_rows, &self.pool).await
    }

    /// Load all slots (with constraints) for a given offer inside a transaction.
    async fn load_slots_for_offer_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        offer_id: Uuid,
    ) -> Result<Vec<OfferSlot>> {
        let slot_rows = sqlx::query_as!(
            OfferSlotRow,
            r#"
            SELECT
                id,
                offer_id,
                label as "label: Name",
                min_items,
                max_items
            FROM offer_slots
            WHERE offer_id = $1
            ORDER BY id
            "#,
            offer_id,
        )
        .fetch_all(&mut **tx)
        .await?;

        // Load all constraints for these slots in one query
        let slot_ids: Vec<Uuid> = slot_rows.iter().map(|s| s.id).collect();
        if slot_ids.is_empty() {
            return Ok(vec![]);
        }

        let constraint_rows = sqlx::query_as!(
            OfferSlotConstraintRow,
            r#"
            SELECT id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id
            FROM offer_slot_constraints
            WHERE slot_id = ANY($1)
            ORDER BY id
            "#,
            &slot_ids,
        )
        .fetch_all(&mut **tx)
        .await?;

        self.assemble_slots_with_constraints(slot_rows, constraint_rows)
    }

    /// Given slot rows, batch-load all their constraints and assemble domain objects.
    async fn slot_rows_with_constraints<'e, E>(
        &self,
        slot_rows: Vec<OfferSlotRow>,
        executor: E,
    ) -> Result<Vec<OfferSlot>>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let slot_ids: Vec<Uuid> = slot_rows.iter().map(|s| s.id).collect();
        if slot_ids.is_empty() {
            return Ok(vec![]);
        }

        let constraint_rows = sqlx::query_as!(
            OfferSlotConstraintRow,
            r#"
            SELECT id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id
            FROM offer_slot_constraints
            WHERE slot_id = ANY($1)
            ORDER BY id
            "#,
            &slot_ids,
        )
        .fetch_all(executor)
        .await?;

        self.assemble_slots_with_constraints(slot_rows, constraint_rows)
    }

    /// Assemble slot rows + constraint rows into domain `OfferSlot` objects.
    fn assemble_slots_with_constraints(
        &self,
        slot_rows: Vec<OfferSlotRow>,
        constraint_rows: Vec<OfferSlotConstraintRow>,
    ) -> Result<Vec<OfferSlot>> {
        // Group constraints by slot_id
        let mut constraints_map: std::collections::HashMap<Uuid, Vec<OfferSlotConstraint>> =
            std::collections::HashMap::new();
        for row in constraint_rows {
            let constraint = self.constraint_row_to_domain(row)?;
            constraints_map
                .entry(constraint.slot_id)
                .or_default()
                .push(constraint);
        }

        let slots = slot_rows
            .into_iter()
            .map(|row| {
                let constraints = constraints_map.remove(&row.id).unwrap_or_default();
                self.slot_row_to_domain(row, constraints)
            })
            .collect();

        Ok(slots)
    }

    /// Convert multiple offer rows into full Offer domain objects with nested slots.
    async fn rows_to_offers_with_slots(&self, rows: Vec<OfferRow>) -> Result<Vec<Offer>> {
        if rows.is_empty() {
            return Ok(vec![]);
        }

        let offer_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();

        // Batch-load all slots for all offers
        let slot_rows = sqlx::query_as!(
            OfferSlotRow,
            r#"
            SELECT
                id,
                offer_id,
                label as "label: Name",
                min_items,
                max_items
            FROM offer_slots
            WHERE offer_id = ANY($1)
            ORDER BY id
            "#,
            &offer_ids,
        )
        .fetch_all(&self.pool)
        .await?;

        // Batch-load all constraints for all those slots
        let slot_ids: Vec<Uuid> = slot_rows.iter().map(|s| s.id).collect();
        let constraint_rows = if slot_ids.is_empty() {
            vec![]
        } else {
            sqlx::query_as!(
                OfferSlotConstraintRow,
                r#"
                SELECT id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id
                FROM offer_slot_constraints
                WHERE slot_id = ANY($1)
                ORDER BY id
                "#,
                &slot_ids,
            )
            .fetch_all(&self.pool)
            .await?
        };

        // Group constraints by slot_id
        let mut constraints_map: std::collections::HashMap<Uuid, Vec<OfferSlotConstraint>> =
            std::collections::HashMap::new();
        for row in constraint_rows {
            let constraint = self.constraint_row_to_domain(row)?;
            constraints_map
                .entry(constraint.slot_id)
                .or_default()
                .push(constraint);
        }

        // Group slots by offer_id
        let mut slots_map: std::collections::HashMap<Uuid, Vec<OfferSlot>> =
            std::collections::HashMap::new();
        for slot_row in slot_rows {
            let constraints = constraints_map.remove(&slot_row.id).unwrap_or_default();
            let slot = self.slot_row_to_domain(slot_row.clone(), constraints);
            slots_map.entry(slot_row.offer_id).or_default().push(slot);
        }

        // Assemble offers
        let offers = rows
            .into_iter()
            .map(|row| {
                let slots = slots_map.remove(&row.id).unwrap_or_default();
                self.row_to_offer(row, slots)
            })
            .collect();

        Ok(offers)
    }

    // ==================== ROW → DOMAIN CONVERSIONS ====================

    fn row_to_offer(&self, row: OfferRow, slots: Vec<OfferSlot>) -> Offer {
        Offer {
            id: row.id,
            restaurant_id: row.restaurant_id,
            menu_id: row.menu_id,
            title: row.title,
            description: row.description,
            fixed_price_cents: row.fixed_price_cents,
            is_active: row.is_active,
            created_at: row.created_at,
            created_by: row.created_by,
            slots,
        }
    }

    fn slot_row_to_domain(
        &self,
        row: OfferSlotRow,
        constraints: Vec<OfferSlotConstraint>,
    ) -> OfferSlot {
        OfferSlot {
            id: row.id,
            offer_id: row.offer_id,
            label: row.label,
            min_items: row.min_items,
            max_items: row.max_items,
            constraints,
        }
    }

    fn constraint_row_to_domain(
        &self,
        row: OfferSlotConstraintRow,
    ) -> Result<OfferSlotConstraint> {
        let kind = SlotConstraintKind::from_db(
            row.allowed_item_id,
            row.allowed_tag_id,
            row.allowed_section_id,
        )?;
        Ok(OfferSlotConstraint {
            id: row.id,
            slot_id: row.slot_id,
            kind,
        })
    }
}

// ==================== Conversion helpers for update replace-all ====================

/// When an `UpdateOfferSlot` is provided during replace-all, we need to convert it
/// into a `CreateOfferSlot` to re-create it. The `id` from the update variant is
/// ignored because we delete and recreate.
impl From<&crate::features::offer::domain::UpdateOfferSlot> for CreateOfferSlot {
    fn from(update: &crate::features::offer::domain::UpdateOfferSlot) -> Self {
        // For replace-all semantics during update: label and min/max are required
        // when replacing slots. If they were optional in the update DTO we
        // unwrap with sensible defaults — but callers should always provide them.
        CreateOfferSlot {
            label: update
                .label
                .clone()
                .expect("label is required when replacing slots"),
            min_items: update
                .min_items
                .expect("min_items is required when replacing slots"),
            max_items: update
                .max_items
                .expect("max_items is required when replacing slots"),
            constraints: update
                .constraints
                .as_ref()
                .map(|cs| {
                    cs.iter()
                        .map(|c| CreateOfferSlotConstraint {
                            kind: c.kind.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}