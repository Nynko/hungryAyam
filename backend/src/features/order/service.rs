use anyhow::{anyhow, Result};
use chrono::{NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;
use uuid::Uuid;
use sqlx::PgPool;

use crate::types::role::UserRole;
use crate::features::offer::service::OfferService;
use crate::features::order::{
    domain::{
        order::{CreateOrder, Order},
        order_session::{
            CreateOrderSession, OrderSession, OrderSessionStatus,
        },
        order_settings::RestaurantOrderSettings,
    },
    dto::{
        OrderSummary, RegularItemSummary, OfferItemCount, OfferSlotSummary,
        OfferGroupSummary, SessionOrderSummary, UpdateOrderSessionRequest,
        UpdateOrderSettingsRequest,
    },
    repository::OrderRepository,
};

/// Aggregate all orders for a session into a structured summary.
///
/// Mirrors the frontend's `aggregatedRegularItems` + `aggregatedOfferGroups`
/// logic exactly so that the UI, email, and SMS all show the same data.
pub async fn aggregate_session_summary(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<SessionOrderSummary> {
    // Fetch all order items in one query
    let rows = sqlx::query!(
        r#"
        SELECT
            o.id              AS order_id,
            o.offer_id        AS "offer_id?: Uuid",
            off.title         AS "offer_title?: String",
            i.id              AS item_id,
            i.name            AS item_name,
            ofs.label         AS "slot_label?: String",
            ofs.slot_group    AS "slot_group?: String",
            ofs.position      AS "slot_position?: i32",
            oi.notes          AS "notes?: String"
        FROM orders o
        JOIN order_items oi ON oi.order_id = o.id
        JOIN items i ON i.id = oi.item_id
        LEFT JOIN offer_slots ofs ON ofs.id = oi.slot_id
        LEFT JOIN offers off ON off.id = o.offer_id
        WHERE o.session_id = $1
        ORDER BY o.created_at, o.id, ofs.position, oi.id
        "#,
        session_id,
    )
    .fetch_all(pool)
    .await?;

    // ── Regular items (no offer) ──────────────────────────────────
    // key: (item_id, note)
    let mut regular_map: HashMap<(Uuid, String), RegularItemSummary> = HashMap::new();

    for row in rows.iter().filter(|r| r.offer_id.is_none()) {
        let note = row.notes.clone();
        let key = (row.item_id, note.clone().unwrap_or_default());
        let entry = regular_map.entry(key).or_insert(RegularItemSummary {
            item_name: row.item_name.clone(),
            quantity: 0,
            note: note.clone(),
        });
        entry.quantity += 1;
    }

    // Sort: plain items first, then by name
    let mut regular_items: Vec<RegularItemSummary> = regular_map.into_values().collect();
    regular_items.sort_by(|a, b| {
        match (&a.note, &b.note) {
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            _ => a.item_name.cmp(&b.item_name),
        }
    });

    // ── Offer groups ──────────────────────────────────────────────
    // Group rows by order_id first, then aggregate per offer
    struct SlotAgg {
        label: String,
        grouped: bool,
        position: i32,                  // from offer_slots.position — used for display order
        combos: HashMap<String, i64>,   // combo string → count (grouped slots)
        items: HashMap<String, i64>,    // item name → count (ungrouped slots)
    }
    struct OfferAgg {
        title: String,
        count: i64,
        slots: HashMap<String, SlotAgg>, // slot_key → agg
    }

    let mut offer_map: HashMap<Uuid, OfferAgg> = HashMap::new();

    // Collect all offer rows grouped by order_id
    let mut order_rows: HashMap<Uuid, Vec<_>> = HashMap::new();
    for row in rows.iter().filter(|r| r.offer_id.is_some()) {
        order_rows.entry(row.order_id).or_default().push(row);
    }

    for (_, items) in &order_rows {
        let first = &items[0];
        let offer_id = first.offer_id.unwrap();
        let title = first.offer_title.clone().unwrap_or_else(|| format!("Offer ({})", &offer_id.to_string()[..8]));

        let agg = offer_map.entry(offer_id).or_insert(OfferAgg {
            title,
            count: 0,
            slots: HashMap::new(),
        });
        agg.count += 1;

        // Partition this order's items by slot_key → slot_label → (position, item names)
        // Position is tracked per slot_label so combo parts and labels respect slot order.
        let mut order_slots: HashMap<String, (bool, i32, HashMap<String, (i32, Vec<String>)>)> = HashMap::new();
        for row in items.iter() {
            let slot_key = row.slot_group.clone()
                .or_else(|| row.slot_label.clone())
                .unwrap_or_else(|| "—".to_string());
            let is_grouped = row.slot_group.is_some();
            let slot_label = row.slot_label.clone().unwrap_or_else(|| "—".to_string());
            let position = row.slot_position.unwrap_or(0);

            let entry = order_slots.entry(slot_key).or_insert((is_grouped, position, HashMap::new()));
            // Slot key position = minimum position of its member slot labels
            if position < entry.1 { entry.1 = position; }
            entry.2.entry(slot_label).or_insert((position, Vec::new())).1.push(row.item_name.clone());
        }

        // Merge into the offer aggregate
        for (slot_key, (is_grouped, position, by_label)) in order_slots {
            // Build display label: slot labels sorted by their position
            let label: String = {
                let mut labels: Vec<(i32, String)> = by_label.iter()
                    .map(|(lbl, (pos, _))| (*pos, lbl.clone()))
                    .collect();
                labels.sort_by_key(|(pos, _)| *pos);
                labels.into_iter().map(|(_, lbl)| lbl).collect::<Vec<_>>().join(" + ")
            };

            let slot_agg = agg.slots.entry(slot_key).or_insert(SlotAgg {
                label,
                grouped: is_grouped,
                position,
                combos: HashMap::new(),
                items: HashMap::new(),
            });
            if position < slot_agg.position { slot_agg.position = position; }

            if is_grouped {
                // Build combo string: parts ordered by slot position, joined with " + "
                let mut parts: Vec<(i32, String)> = by_label.into_iter()
                    .map(|(_, (pos, names))| (pos, names.join(" & ")))
                    .collect();
                parts.sort_by_key(|(pos, _)| *pos);
                let combo = parts.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(" + ");
                *slot_agg.combos.entry(combo).or_insert(0) += 1;
            } else {
                for (_, (_, names)) in by_label {
                    for name in names {
                        *slot_agg.items.entry(name).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Convert to output DTOs
    let mut offer_groups: Vec<OfferGroupSummary> = offer_map.into_values().map(|agg| {
        // Sort slot aggregates by position before converting to DTOs
        let mut slot_aggs: Vec<SlotAgg> = agg.slots.into_values().collect();
        slot_aggs.sort_by_key(|s| s.position);

        let slots: Vec<OfferSlotSummary> = slot_aggs.into_iter().map(|s| {
            let mut items: Vec<OfferItemCount> = if s.grouped {
                s.combos.into_iter().map(|(name, qty)| OfferItemCount { name, qty }).collect()
            } else {
                s.items.into_iter().map(|(name, qty)| OfferItemCount { name, qty }).collect()
            };
            items.sort_by(|a, b| a.name.cmp(&b.name));
            OfferSlotSummary { label: s.label, items }
        }).collect();
        OfferGroupSummary { offer_title: agg.title, count: agg.count, slots }
    }).collect();
    offer_groups.sort_by(|a, b| a.offer_title.cmp(&b.offer_title));

    Ok(SessionOrderSummary { regular_items, offer_groups })
}

#[derive(Clone)]
pub struct OrderService {
    repository: OrderRepository,
    offer_service: OfferService,
    /// Shared handle used to wake the background scheduler when data it cares
    /// about changes (e.g. session created/updated/closed, order settings
    /// changed).
    scheduler_notify: Arc<Notify>,
}

impl OrderService {
    pub fn new(repository: OrderRepository, offer_service: OfferService, scheduler_notify: Arc<Notify>) -> Self {
        Self { repository, offer_service, scheduler_notify }
    }

    // ==================== ORDER SESSION OPERATIONS ====================

    /// Create a new order session for a restaurant.
    ///
    /// The session starts in `Open` status. Multiple open sessions are allowed
    /// so that users can order for different pickup times concurrently.
    pub async fn create_session(
        &self,
        request: CreateOrderSession,
        user_id: Uuid,
    ) -> Result<OrderSession> {
        // Validate: end_date must be after start_date
        if request.end_date <= request.start_date {
            return Err(anyhow!(
                "Session end_date must be after start_date"
            ));
        }

        let session = self.repository.create_session(request, user_id).await?;
        // Wake the scheduler — a new session's end_date may be earlier than
        // the current sleep target.
        self.scheduler_notify.notify_one();
        Ok(session)
    }

    /// Get an order session by ID (without orders).
    pub async fn get_session(&self, id: Uuid) -> Result<Option<OrderSession>> {
        self.repository.get_session_by_id(id).await
    }

    /// Get an order session by ID with all its orders and order items.
    pub async fn get_session_with_orders(&self, id: Uuid) -> Result<Option<OrderSession>> {
        self.repository.get_session_with_orders(id).await
    }

    /// List all sessions for a restaurant (most recent first, without orders).
    pub async fn list_sessions_by_restaurant(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Vec<OrderSession>> {
        self.repository
            .list_sessions_by_restaurant(restaurant_id)
            .await
    }

    /// Get the currently active (Open) session for a restaurant, if any.
    pub async fn get_active_session(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        self.repository
            .get_active_session_for_restaurant(restaurant_id)
            .await
    }

    /// List all open sessions for a restaurant, ordered by pickup time (end_date asc).
    pub async fn list_open_sessions(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Vec<OrderSession>> {
        self.repository
            .list_open_sessions_for_restaurant(restaurant_id)
            .await
    }

    /// Move an order to a different session.
    ///
    /// Validates that the user owns the order (or is an editor), that the
    /// current session is Open, and that the target session is Open and belongs
    /// to the same restaurant. After moving, auto-deletes the old session if it
    /// has no remaining orders.
    pub async fn move_order_to_session(
        &self,
        order_id: Uuid,
        new_session_id: Uuid,
        user_id: Uuid,
        user_role: Option<UserRole>,
    ) -> Result<Order> {
        let order = self
            .repository
            .get_order_by_id(order_id)
            .await?
            .ok_or_else(|| anyhow!("Order not found"))?;

        let is_owner = order.user_id == user_id;
        let is_editor_or_above = user_role.as_ref().map_or(false, |r| r.is_editor_or_above());
        if !is_owner && !is_editor_or_above {
            return Err(anyhow!("You can only move your own orders"));
        }

        let old_session_id = order
            .session_id
            .ok_or_else(|| anyhow!("Order has no session_id"))?;

        if old_session_id == new_session_id {
            return Ok(order);
        }

        let old_session = self
            .repository
            .get_session_by_id(old_session_id)
            .await?
            .ok_or_else(|| anyhow!("Current session not found"))?;

        if !old_session.status.is_accepting_orders() {
            return Err(anyhow!(
                "Cannot move an order from a session in '{}' status",
                old_session.status
            ));
        }

        let new_session = self
            .repository
            .get_session_by_id(new_session_id)
            .await?
            .ok_or_else(|| anyhow!("Target session not found"))?;

        if new_session.restaurant_id != old_session.restaurant_id {
            return Err(anyhow!("Target session belongs to a different restaurant"));
        }

        if !new_session.status.is_accepting_orders() {
            return Err(anyhow!(
                "Target session '{}' is not accepting orders (status: '{}')",
                new_session_id,
                new_session.status
            ));
        }

        self.repository
            .move_order_to_session(order_id, new_session_id)
            .await?;

        // Auto-delete old session if it is now empty
        let remaining = self.repository.count_orders_in_session(old_session_id).await?;
        if remaining == 0 {
            let _ = self.repository.delete_session_unconditionally(old_session_id).await;
            self.scheduler_notify.notify_one();
        }

        let updated_order = self
            .repository
            .get_order_by_id(order_id)
            .await?
            .ok_or_else(|| anyhow!("Order not found after move"))?;

        Ok(updated_order)
    }

    /// List all orders placed by a specific user across all open sessions for a restaurant.
    pub async fn list_orders_by_user_in_open_sessions(
        &self,
        restaurant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Order>> {
        self.repository
            .list_orders_by_user_in_open_sessions(restaurant_id, user_id)
            .await
    }

    /// Update mutable fields of a session (start_date, end_date, allow_late).
    ///
    /// Only sessions in non-terminal states (Open or Closed) can be edited.
    /// Terminal states (Cancelled, Sent) are immutable.
    pub async fn update_session(
        &self,
        request: UpdateOrderSessionRequest,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        // Fetch the session to check its current status
        let session = self
            .repository
            .get_session_by_id(request.id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if session.status.is_terminal() {
            return Err(anyhow!(
                "Cannot update a session in '{}' status",
                session.status
            ));
        }

        // If both start and end are provided, validate ordering
        let effective_start = request.start_date.unwrap_or(session.start_date);
        let effective_end = request.end_date.unwrap_or(session.end_date);
        if effective_end <= effective_start {
            return Err(anyhow!(
                "Session end_date must be after start_date"
            ));
        }

        let result = self.repository.update_session(request, user_id).await?;
        // Wake the scheduler — end_date or allow_late may have changed.
        self.scheduler_notify.notify_one();
        Ok(result)
    }

    /// Cancel an order session.
    ///
    /// Transitions from Open or Closed → Cancelled.
    /// Cannot cancel a session that is already in a terminal state.
    pub async fn cancel_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if session.status == OrderSessionStatus::Finished {
            return Err(anyhow!(
                "Cannot cancel a Finished session"
            ));
        }
        if session.status == OrderSessionStatus::Cancelled {
            return Err(anyhow!(
                "Session is already cancelled"
            ));
        }

        let result = self.repository
            .set_session_status(session_id, OrderSessionStatus::Cancelled, user_id)
            .await?;
        // Wake the scheduler — it can skip this cancelled session now.
        self.scheduler_notify.notify_one();
        Ok(result)
    }

    /// Close an order session (stop accepting new orders).
    ///
    /// Transitions from Open → Closed.
    pub async fn close_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if session.status != OrderSessionStatus::Open {
            return Err(anyhow!(
                "Can only close a session that is Open. Current status: '{}'",
                session.status
            ));
        }

        let result = self.repository
            .set_session_status(session_id, OrderSessionStatus::Closed, user_id)
            .await?;
        // Wake the scheduler — it can skip this closed session now.
        self.scheduler_notify.notify_one();
        Ok(result)
    }

    /// Send an order request to the restaurant (SMS / WhatsApp / email).
    ///
    /// Transitions from Closed → Requested.
    /// The session must be closed before requesting to prevent new orders sneaking in.
    pub async fn request_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if session.status != OrderSessionStatus::Closed {
            return Err(anyhow!(
                "Can only request a session that is Closed. Current status: '{}'.",
                session.status
            ));
        }

        self.repository
            .set_session_status(session_id, OrderSessionStatus::Requested, user_id)
            .await
    }

    /// Confirm that the restaurant will fulfil the order.
    ///
    /// Transitions from Closed, Requested, or SmsSent → Confirmed.
    /// Manual flow: Closed → Confirmed (skipping the SMS step).
    /// SMS flow: Requested or SmsSent → Confirmed (restaurant acknowledged).
    pub async fn confirm_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if !matches!(session.status,
            OrderSessionStatus::Closed |
            OrderSessionStatus::Requested |
            OrderSessionStatus::SmsSent
        ) {
            return Err(anyhow!(
                "Can only confirm a session that is Closed, Requested, or SmsSent. Current status: '{}'.",
                session.status
            ));
        }

        self.repository
            .set_session_status(session_id, OrderSessionStatus::Confirmed, user_id)
            .await
    }

    /// Mark a session as finished (food picked up / delivered).
    ///
    /// Transitions from Confirmed → Finished.
    /// This can also be triggered automatically by the scheduler after pickup_time.
    pub async fn finish_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if session.status != OrderSessionStatus::Confirmed {
            return Err(anyhow!(
                "Can only finish a session that is Confirmed. Current status: '{}'.",
                session.status
            ));
        }

        self.repository
            .set_session_status(session_id, OrderSessionStatus::Finished, user_id)
            .await
    }

    /// Reopen a closed session (allow orders again).
    ///
    /// Transitions from Closed → Open. Cannot reopen terminal states.
    pub async fn reopen_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if session.status != OrderSessionStatus::Closed {
            return Err(anyhow!(
                "Can only reopen a session that is Closed. Current status: '{}'",
                session.status
            ));
        }

        let result = self.repository
            .set_session_status(session_id, OrderSessionStatus::Open, user_id)
            .await?;
        // Wake the scheduler — a reopened session may need auto-close tracking.
        self.scheduler_notify.notify_one();
        Ok(result)
    }

    /// Delete a session. Only sessions in Cancelled status can be deleted.
    pub async fn delete_session(&self, session_id: Uuid) -> Result<bool> {
        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Session not found"))?;

        if session.status != OrderSessionStatus::Cancelled {
            return Err(anyhow!(
                "Can only delete a cancelled session. Current status: '{}'. \
                 Cancel the session first.",
                session.status
            ));
        }

        self.repository.delete_session(session_id).await
    }

    // ==================== ORDER OPERATIONS ====================

    /// Create an order within an order session.
    ///
    /// If `request.session_id` is `Some`, the order is placed in that specific
    /// session (which must be Open).
    ///
    /// If `request.session_id` is `None`, the service tries to find an active
    /// session for the restaurant. If none exists and the restaurant's order
    /// settings have `auto_create_session` enabled, a new session is created
    /// automatically using the restaurant's default timing.
    ///
    /// The total price is computed server-side from the items' base prices.
    pub async fn create_order(
        &self,
        request: CreateOrder,
        user_id: Uuid,
    ) -> Result<Order> {
        // Validate: at least one item
        if request.items.is_empty() {
            return Err(anyhow!("An order must contain at least one item"));
        }

        // Validate: all item IDs exist and belong to the restaurant
        let item_ids: Vec<Uuid> = request.items.iter().map(|i| i.item_id).collect();
        let all_valid = self
            .repository
            .validate_item_ids(&item_ids, request.restaurant_id)
            .await?;
        if !all_valid {
            return Err(anyhow!(
                "One or more item IDs are invalid or do not belong to the specified restaurant"
            ));
        }

        // Resolve the session
        let session = self
            .resolve_session(request.restaurant_id, request.session_id, user_id)
            .await?;

        // Check that the session is accepting orders
        if !session.status.is_accepting_orders() {
            return Err(anyhow!(
                "Session '{}' is not accepting orders (status: '{}')",
                session.id,
                session.status
            ));
        }

        // Check that the session hasn't expired (unless allow_late is true)
        let now = Utc::now();
        if now > session.end_date && !session.allow_late {
            return Err(anyhow!(
                "Session '{}' has expired. The order deadline was {}.",
                session.id,
                session.end_date
            ));
        }

        // Compute total price — offer-based or item-sum
        let (total_price_cents, validated_offer_id) = if let Some(offer_id) = request.offer_id {
            // Validate the offer and all slot selections
            let items_with_slots: Vec<(Uuid, Option<Uuid>)> = request
                .items
                .iter()
                .map(|i| (i.item_id, i.slot_id))
                .collect();

            let offer = self
                .offer_service
                .validate_offer_order(offer_id, request.restaurant_id, &items_with_slots)
                .await?;

            // Compute offer price: base + slot supplements + constraint supplements
            let total = self
                .offer_service
                .compute_offer_price(&offer, &items_with_slots)
                .await?;
            (total, Some(offer_id))
        } else {
            // No offer — compute total from item base prices.
            // For items that appear multiple times, we sum each occurrence individually.
            let total = self.compute_order_total(&item_ids).await?;
            (total, None)
        };

        // Create the order with items
        self.repository
            .create_order(
                request.restaurant_id,
                session.id,
                user_id,
                total_price_cents,
                validated_offer_id,
                &request.items,
            )
            .await
    }

    /// Get an order by ID (with items).
    pub async fn get_order(&self, id: Uuid) -> Result<Option<Order>> {
        self.repository.get_order_by_id(id).await
    }

    /// List all orders in a session (with items).
    pub async fn list_orders_by_session(&self, session_id: Uuid) -> Result<Vec<Order>> {
        self.repository.list_orders_by_session(session_id).await
    }

    /// List orders placed by a specific user in a session (with items).
    pub async fn list_orders_by_user_in_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Order>> {
        self.repository
            .list_orders_by_user_in_session(session_id, user_id)
            .await
    }

    /// Get lightweight order summaries for a session.
    pub async fn list_order_summaries(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<OrderSummary>> {
        self.repository
            .list_order_summaries_by_session(session_id)
            .await
    }

    /// Delete an order.
    ///
    /// Orders can only be deleted while the parent session is still Open.
    /// Once closed, requested, confirmed, or finished, orders are immutable.
    pub async fn delete_order(
        &self,
        order_id: Uuid,
        user_id: Uuid,
        user_role: Option<UserRole>,
    ) -> Result<bool> {
        let order = self
            .repository
            .get_order_by_id(order_id)
            .await?
            .ok_or_else(|| anyhow!("Order not found"))?;

        // Only the order owner or an editor/admin can delete an order
        let is_owner = order.user_id == user_id;
        let is_editor_or_above = user_role.as_ref().map_or(false, |r| r.is_editor_or_above());
        if !is_owner && !is_editor_or_above {
            return Err(anyhow!("You can only delete your own orders"));
        }

        let session_id = order
            .session_id
            .ok_or_else(|| anyhow!("Order has no session_id"))?;

        let session = self
            .repository
            .get_session_by_id(session_id)
            .await?
            .ok_or_else(|| anyhow!("Parent session not found"))?;

        if !session.status.is_accepting_orders() {
            return Err(anyhow!(
                "Cannot delete an order from a session in '{}' status. \
                 Orders can only be deleted while the session is Open.",
                session.status
            ));
        }

        let deleted = self.repository.delete_order(order_id).await?;

        // Auto-delete the session if it is now empty
        if deleted {
            let remaining = self.repository.count_orders_in_session(session_id).await?;
            if remaining == 0 {
                let _ = self.repository.delete_session_unconditionally(session_id).await;
                self.scheduler_notify.notify_one();
            }
        }

        Ok(deleted)
    }

    // ==================== ORDER SETTINGS OPERATIONS ====================

    /// Get order settings for a restaurant. Returns settings with defaults if
    /// none have been explicitly configured.
    pub async fn get_order_settings(
        &self,
        restaurant_id: Uuid,
    ) -> Result<RestaurantOrderSettings> {
        self.repository
            .get_or_create_settings(restaurant_id)
            .await
    }

    /// Update order settings for a restaurant. Creates default settings first
    /// if they don't exist, then applies the update.
    pub async fn update_order_settings(
        &self,
        request: UpdateOrderSettingsRequest,
    ) -> Result<Option<RestaurantOrderSettings>> {
        // Validate times if both are provided
        if let (Some(start), Some(end)) = (request.default_start_time, request.default_end_time) {
            if end <= start {
                return Err(anyhow!(
                    "default_end_time must be after default_start_time"
                ));
            }
        }

        let result = self.repository.update_settings(request).await?;
        // Wake the scheduler — menu_reset_time or auto_close_session may have
        // changed, requiring a recalculated sleep target.
        self.scheduler_notify.notify_one();
        Ok(result)
    }

    // ==================== PRIVATE HELPERS ====================

    /// Resolve which session to use for an order.
    ///
    /// 1. If an explicit `session_id` is given, fetch and return it.
    /// 2. Otherwise, look for an active session for the restaurant.
    /// 3. If no active session exists and `auto_create_session` is enabled,
    ///    create one using the restaurant's default timing.
    /// 4. Otherwise, return an error.
    async fn resolve_session(
        &self,
        restaurant_id: Uuid,
        explicit_session_id: Option<Uuid>,
        user_id: Uuid,
    ) -> Result<OrderSession> {
        // Case 1: Explicit session ID
        if let Some(session_id) = explicit_session_id {
            let session = self
                .repository
                .get_session_by_id(session_id)
                .await?
                .ok_or_else(|| anyhow!("Specified session not found (id: {})", session_id))?;

            // Verify the session belongs to the correct restaurant
            if session.restaurant_id != restaurant_id {
                return Err(anyhow!(
                    "Session '{}' belongs to a different restaurant",
                    session_id
                ));
            }

            return Ok(session);
        }

        // Case 2: Look for Open sessions (can accept orders)
        let open_sessions: Vec<_> = self
            .repository
            .list_open_sessions_for_restaurant(restaurant_id)
            .await?
            .into_iter()
            .filter(|s| s.status == OrderSessionStatus::Open)
            .collect();

        match open_sessions.len() {
            1 => return Ok(open_sessions.into_iter().next().unwrap()),
            n if n > 1 => {
                return Err(anyhow!(
                    "Multiple pickup sessions are available. Please select which session to order in."
                ));
            }
            _ => {} // zero sessions — fall through to auto-create
        }

        // Case 3: Auto-create if enabled
        let settings = self
            .repository
            .get_or_create_settings(restaurant_id)
            .await?;

        if !settings.auto_create_session {
            return Err(anyhow!(
                "No active session for this restaurant and auto-creation is disabled. \
                 Please create a session manually."
            ));
        }

        let session = self
            .auto_create_session(restaurant_id, &settings, user_id)
            .await?;

        Ok(session)
    }

    /// Auto-create a session using the restaurant's default timing.
    ///
    /// Creates a session for today with start and end times derived from the
    /// restaurant's order settings. If the default end time has already passed
    /// today, the session is created for tomorrow.
    async fn auto_create_session(
        &self,
        restaurant_id: Uuid,
        settings: &RestaurantOrderSettings,
        user_id: Uuid,
    ) -> Result<OrderSession> {
        let now = Utc::now();

        // Parse the restaurant's IANA timezone; fall back to UTC if invalid.
        let tz: Tz = settings.timezone.parse().unwrap_or(chrono_tz::UTC);

        // "Today" in the restaurant's local timezone.
        let now_local = now.with_timezone(&tz);
        let today_local = now_local.date_naive();

        // Build start/end as local naive datetimes, then convert to UTC.
        let naive_start = today_local.and_time(settings.default_start_time);
        let naive_end   = today_local.and_time(settings.default_end_time);

        let start_dt = tz.from_local_datetime(&naive_start)
            .single()
            .ok_or_else(|| anyhow!("Ambiguous or invalid local start time"))?
            .with_timezone(&Utc);
        let end_dt = tz.from_local_datetime(&naive_end)
            .single()
            .ok_or_else(|| anyhow!("Ambiguous or invalid local end time"))?
            .with_timezone(&Utc);

        // If the end time has already passed, schedule for tomorrow.
        let (start_dt, end_dt) = if end_dt <= now {
            let tomorrow = today_local.succ_opt().ok_or_else(|| anyhow!("Date overflow"))?;
            let s = tz.from_local_datetime(&tomorrow.and_time(settings.default_start_time))
                .single()
                .ok_or_else(|| anyhow!("Ambiguous or invalid local start time (tomorrow)"))?
                .with_timezone(&Utc);
            let e = tz.from_local_datetime(&tomorrow.and_time(settings.default_end_time))
                .single()
                .ok_or_else(|| anyhow!("Ambiguous or invalid local end time (tomorrow)"))?
                .with_timezone(&Utc);
            (s, e)
        } else {
            (start_dt, end_dt)
        };

        // Default pickup time: use `default_pickup_time` setting if configured,
        // otherwise fall back to 12:15 local time on the session day.
        let pickup_naive_time = settings.default_pickup_time
            .unwrap_or_else(|| NaiveTime::from_hms_opt(12, 15, 0).expect("valid time"));
        let pickup_day = end_dt.with_timezone(&tz).date_naive();
        let default_pickup = tz
            .from_local_datetime(&pickup_day.and_time(pickup_naive_time))
            .single()
            .ok_or_else(|| anyhow!("Ambiguous or invalid pickup time"))?
            .with_timezone(&Utc);

        let create_request = CreateOrderSession {
            restaurant_id,
            start_date: start_dt,
            end_date: end_dt,
            pickup_time: Some(default_pickup),
            allow_late: false,
        };

        let session = self.repository.create_session(create_request, user_id).await?;
        // Wake the scheduler — a new auto-created session needs tracking.
        self.scheduler_notify.notify_one();
        Ok(session)
    }

    /// Compute the total price for a non-offer order.
    ///
    /// Sums the base_price_cents of each item. Items that appear multiple times
    /// in the order are counted multiple times.
    async fn compute_order_total(&self, item_ids: &[Uuid]) -> Result<i32> {
        // Items may appear multiple times. We need to look up the price of each
        // unique item and then multiply by occurrence count.
        let mut id_counts: std::collections::HashMap<Uuid, i32> = std::collections::HashMap::new();
        for id in item_ids {
            *id_counts.entry(*id).or_insert(0) += 1;
        }

        // If every item appears exactly once, use the fast-path batch lookup.
        if id_counts.values().all(|&c| c == 1) {
            return self.repository.compute_total_price_cents(item_ids).await;
        }

        // Slow path: has duplicates — fetch individual prices and scale.
        // This could be optimised but duplicates are expected to be rare.
        let mut total: i32 = 0;
        for (&item_id, &count) in &id_counts {
            let item_total = self
                .repository
                .compute_total_price_cents(&[item_id])
                .await?;
            total += item_total * count;
        }

        Ok(total)
    }
}