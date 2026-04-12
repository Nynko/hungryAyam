use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::{
        availability::{
            db_model::AvailabilityRuleRow,
            domain::AvailabilityRule,
        },
        offer::{
            db_model::{OfferRow, OfferSlotConstraintRow, OfferSlotRow},
            domain::{
                CreateOffer, CreateOfferSlot, CreateOfferSlotConstraint, Offer, OfferSlot,
                OfferSlotConstraint, SlotConstraintKind, UpdateOffer,
            },
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
            INSERT INTO offers (restaurant_id, menu_id, title, description, base_price_cents, is_active, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                is_active,
                created_at,
                created_by,
                availability_rule_id
            "#,
            request.restaurant_id,
            request.menu_id,
            request.title.as_ref(),
            request.description,
            request.base_price_cents.as_ref(),
            request.is_active,
            user_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        let mut slots = Vec::with_capacity(request.slots.len());
        for (pos, slot_req) in request.slots.iter().enumerate() {
            let slot = self
                .create_slot_in_tx(&mut tx, offer_row.id, pos as i32, slot_req)
                .await?;
            slots.push(slot);
        }

        tx.commit().await?;

        Ok(self.row_to_offer(offer_row, slots, None))
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
                base_price_cents as "base_price_cents: PriceCents",
                is_active,
                created_at,
                created_by,
                availability_rule_id
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
                let rule = self.load_availability_rule(row.availability_rule_id).await?;
                Ok(Some(self.row_to_offer(row, slots, rule)))
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
                base_price_cents as "base_price_cents: PriceCents",
                is_active,
                created_at,
                created_by,
                availability_rule_id
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
                base_price_cents as "base_price_cents: PriceCents",
                is_active,
                created_at,
                created_by,
                availability_rule_id
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
                base_price_cents = COALESCE($4, base_price_cents),
                is_active    = COALESCE($5, is_active)
            WHERE id = $6
            RETURNING
                id,
                restaurant_id,
                menu_id,
                title as "title: Name",
                description,
                base_price_cents as "base_price_cents: PriceCents",
                is_active,
                created_at,
                created_by,
                availability_rule_id
            "#,
            request.menu_id,
            request.title.as_ref().map(|n| n.as_ref()),
            request.description,
            request.base_price_cents.as_ref().map(|p| p.as_ref()),
            request.is_active,
            request.id,
        )
        .fetch_optional(&mut *tx)
        .await?;

        let offer_row = match offer_row {
            Some(r) => r,
            None => return Ok(None),
        };

        // If slots were provided, upsert: update existing, insert new, delete removed.
        let slots = if let Some(new_slots) = request.slots {
            // Collect IDs of slots being kept/updated.
            let kept_ids: Vec<Uuid> = new_slots.iter().filter_map(|s| s.id).collect();

            // Delete slots no longer present (ON DELETE SET NULL handles order_items).
            sqlx::query!(
                "DELETE FROM offer_slots WHERE offer_id = $1 AND NOT (id = ANY($2))",
                offer_row.id,
                &kept_ids,
            )
            .execute(&mut *tx)
            .await?;

            let mut slots = Vec::with_capacity(new_slots.len());
            for (pos, slot_req) in new_slots.iter().enumerate() {
                let slot = if let Some(slot_id) = slot_req.id {
                    self.update_slot_in_tx(&mut tx, slot_id, pos as i32, slot_req)
                        .await?
                } else {
                    self.create_slot_in_tx(&mut tx, offer_row.id, pos as i32, &slot_req.into())
                        .await?
                };
                slots.push(slot);
            }
            slots
        } else {
            // Load existing slots
            self.load_slots_for_offer_tx(&mut tx, offer_row.id).await?
        };

        tx.commit().await?;

        let rule = self.load_availability_rule(offer_row.availability_rule_id).await?;
        Ok(Some(self.row_to_offer(offer_row, slots, rule)))
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
                base_price_cents as "base_price_cents: PriceCents",
                is_active,
                created_at,
                created_by,
                availability_rule_id
            "#,
            active,
            id,
        )
        .fetch_optional(&self.pool)
        .await?;

        match offer_row {
            Some(row) => {
                let slots = self.load_slots_for_offer(row.id).await?;
                let rule = self.load_availability_rule(row.availability_rule_id).await?;
                Ok(Some(self.row_to_offer(row, slots, rule)))
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

    /// For a given slot and a list of item IDs, resolve the *maximum* constraint
    /// supplement (cents) that applies to each item. Returns a mapping of
    /// item_id → supplement_cents.
    ///
    /// When an item matches multiple constraints on the same slot, the constraint
    /// with the **lowest** supplement is used (most favorable to the customer).
    pub async fn get_constraint_supplements_for_items(
        &self,
        slot_id: Uuid,
        item_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, i32>> {
        if item_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let rows = sqlx::query!(
            r#"
            SELECT
                matched.item_id as "item_id!",
                MIN(matched.supplement_cents) as "supplement_cents!"
            FROM (
                -- Directly allowed items
                SELECT osc.allowed_item_id AS item_id, osc.supplement_cents
                FROM offer_slot_constraints osc
                WHERE osc.slot_id = $1
                  AND osc.allowed_item_id IS NOT NULL
                  AND osc.allowed_item_id = ANY($2)

                UNION ALL

                -- Items matching an allowed tag
                SELECT it.item_id, osc.supplement_cents
                FROM offer_slot_constraints osc
                JOIN item_tags it ON it.tag_id = osc.allowed_tag_id
                WHERE osc.slot_id = $1
                  AND osc.allowed_tag_id IS NOT NULL
                  AND it.item_id = ANY($2)

                UNION ALL

                -- Available items in an allowed section
                SELECT msi.item_id, osc.supplement_cents
                FROM offer_slot_constraints osc
                JOIN menu_section_items msi ON msi.section_id = osc.allowed_section_id
                    AND msi.is_available = true
                WHERE osc.slot_id = $1
                  AND osc.allowed_section_id IS NOT NULL
                  AND msi.item_id = ANY($2)
            ) AS matched
            GROUP BY matched.item_id
            "#,
            slot_id,
            item_ids,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut map = std::collections::HashMap::new();
        for row in rows {
            map.insert(row.item_id, row.supplement_cents);
        }
        Ok(map)
    }

    // ==================== PRIVATE HELPERS ====================

    /// Create a single slot with its constraints inside an existing transaction.
    async fn create_slot_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        offer_id: Uuid,
        position: i32,
        request: &CreateOfferSlot,
    ) -> Result<OfferSlot> {
        let slot_row = sqlx::query_as!(
            OfferSlotRow,
            r#"
            INSERT INTO offer_slots (offer_id, label, min_items, max_items, supplement_cents, position, slot_group)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                offer_id,
                label as "label: Name",
                min_items,
                max_items,
                supplement_cents,
                position,
                slot_group
            "#,
            offer_id,
            request.label.as_ref(),
            request.min_items,
            request.max_items,
            request.supplement_cents,
            position,
            request.slot_group,
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

    /// Update an existing slot in place and replace its constraints.
    async fn update_slot_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        slot_id: Uuid,
        position: i32,
        request: &crate::features::offer::domain::UpdateOfferSlot,
    ) -> Result<OfferSlot> {
        let slot_row = sqlx::query_as!(
            OfferSlotRow,
            r#"
            UPDATE offer_slots
            SET
                label           = COALESCE($2, label),
                min_items       = COALESCE($3, min_items),
                max_items       = COALESCE($4, max_items),
                supplement_cents = COALESCE($5, supplement_cents),
                slot_group      = $6,
                position        = $7
            WHERE id = $1
            RETURNING
                id,
                offer_id,
                label as "label: Name",
                min_items,
                max_items,
                supplement_cents,
                position,
                slot_group
            "#,
            slot_id,
            request.label.as_ref().map(|n| n.as_ref()),
            request.min_items,
            request.max_items,
            request.supplement_cents,
            request.slot_group.as_deref(),
            position,
        )
        .fetch_one(&mut **tx)
        .await?;

        // Replace constraints for this slot.
        sqlx::query!("DELETE FROM offer_slot_constraints WHERE slot_id = $1", slot_id)
            .execute(&mut **tx)
            .await?;

        let create_constraints: Vec<CreateOfferSlotConstraint> = request
            .constraints
            .as_ref()
            .map(|cs| {
                cs.iter()
                    .map(|c| CreateOfferSlotConstraint {
                        kind: c.kind.clone(),
                        supplement_cents: c.supplement_cents.unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut constraints = Vec::with_capacity(create_constraints.len());
        for c_req in &create_constraints {
            let constraint = self.create_constraint_in_tx(tx, slot_row.id, c_req).await?;
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
            INSERT INTO offer_slot_constraints (slot_id, allowed_item_id, allowed_tag_id, allowed_section_id, supplement_cents)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id, supplement_cents
            "#,
            slot_id,
            request.kind.item_id(),
            request.kind.tag_id(),
            request.kind.section_id(),
            request.supplement_cents,
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
                max_items,
                supplement_cents,
                position,
                slot_group
            FROM offer_slots
            WHERE offer_id = $1
            ORDER BY position
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
                max_items,
                supplement_cents,
                position,
                slot_group
            FROM offer_slots
            WHERE offer_id = $1
            ORDER BY position
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
            SELECT id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id, supplement_cents
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
            SELECT id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id, supplement_cents
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

    /// Load an availability rule by ID, if present.
    async fn load_availability_rule(&self, rule_id: Option<Uuid>) -> Result<Option<AvailabilityRule>> {
        match rule_id {
            Some(id) => {
                let row = sqlx::query_as!(
                    AvailabilityRuleRow,
                    r#"SELECT id, valid_from, valid_to, start_time, end_time, weekdays, public_holidays_country, public_holidays_mode, active
                       FROM availability_rules WHERE id = $1"#,
                    id
                )
                .fetch_optional(&self.pool)
                .await?;
                Ok(row.map(|r| AvailabilityRule {
                    id: r.id,
                    valid_from: r.valid_from,
                    valid_to: r.valid_to,
                    start_time: r.start_time,
                    end_time: r.end_time,
                    weekdays: r.weekdays,
                        public_holidays_country: r.public_holidays_country,
                        public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s { "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude), "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only), _ => None }),
                    active: r.active,
                }))
            }
            None => Ok(None),
        }
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
                max_items,
                supplement_cents,
                position,
                slot_group
            FROM offer_slots
            WHERE offer_id = ANY($1)
            ORDER BY offer_id, position
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
                SELECT id, slot_id, allowed_item_id, allowed_tag_id, allowed_section_id, supplement_cents
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

        // Batch-load availability rules
        let rule_ids: Vec<Uuid> = rows.iter().filter_map(|r| r.availability_rule_id).collect();
        let mut rules_map: std::collections::HashMap<Uuid, AvailabilityRule> = if !rule_ids.is_empty() {
            let rule_rows = sqlx::query_as!(
                AvailabilityRuleRow,
                r#"SELECT id, valid_from, valid_to, start_time, end_time, weekdays, public_holidays_country, public_holidays_mode, active
                   FROM availability_rules WHERE id = ANY($1)"#,
                &rule_ids
            )
            .fetch_all(&self.pool)
            .await?;
            rule_rows.into_iter().map(|r| {
                (r.id, AvailabilityRule {
                    id: r.id,
                    valid_from: r.valid_from,
                    valid_to: r.valid_to,
                    start_time: r.start_time,
                    end_time: r.end_time,
                    weekdays: r.weekdays,
                        public_holidays_country: r.public_holidays_country,
                        public_holidays_mode: r.public_holidays_mode.as_deref().and_then(|s| match s { "exclude" => Some(crate::features::availability::domain::PublicHolidaysMode::Exclude), "only" => Some(crate::features::availability::domain::PublicHolidaysMode::Only), _ => None }),
                    active: r.active,
                })
            }).collect()
        } else {
            std::collections::HashMap::new()
        };

        // Assemble offers
        let offers = rows
            .into_iter()
            .map(|row| {
                let slots = slots_map.remove(&row.id).unwrap_or_default();
                let rule = row.availability_rule_id.and_then(|rid| rules_map.remove(&rid));
                self.row_to_offer(row, slots, rule)
            })
            .collect();

        Ok(offers)
    }

    // ==================== ROW → DOMAIN CONVERSIONS ====================

    fn row_to_offer(&self, row: OfferRow, slots: Vec<OfferSlot>, availability_rule: Option<AvailabilityRule>) -> Offer {
        Offer {
            id: row.id,
            restaurant_id: row.restaurant_id,
            menu_id: row.menu_id,
            title: row.title,
            description: row.description,
            base_price_cents: row.base_price_cents,
            is_active: row.is_active,
            created_at: row.created_at,
            created_by: row.created_by,
            slots,
            availability_rule,
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
            supplement_cents: row.supplement_cents,
            position: row.position,
            slot_group: row.slot_group,
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
            supplement_cents: row.supplement_cents,
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
            supplement_cents: update.supplement_cents.unwrap_or(0),
            slot_group: update.slot_group.clone(),
            constraints: update
                .constraints
                .as_ref()
                .map(|cs| {
                    cs.iter()
                        .map(|c| CreateOfferSlotConstraint {
                            kind: c.kind.clone(),
                            supplement_cents: c.supplement_cents.unwrap_or(0),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}