use anyhow::{anyhow, Result};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::types::role::UserRole;
use crate::features::offer::service::OfferService;
use crate::features::order::{
    domain::{
        order::{CreateOrder, Order},
        order_session::{
            CreateOrderSession, OrderSession, OrderSessionStatus, UpdateOrderSession,
        },
        order_settings::RestaurantOrderSettings,
    },
    dto::{OrderSummary, UpdateOrderSettingsRequest},
    repository::OrderRepository,
};

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
        request: UpdateOrderSession,
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

        if session.status.is_terminal() {
            return Err(anyhow!(
                "Cannot cancel a session in '{}' status",
                session.status
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

    /// Mark a session as sent (orders have been dispatched to the restaurant).
    ///
    /// Transitions from Closed → Sent. A session must be closed before it
    /// can be marked as sent (to prevent new orders sneaking in).
    pub async fn send_session(
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
                "Can only send a session that is Closed. Current status: '{}'. \
                 Close the session first.",
                session.status
            ));
        }

        self.repository
            .set_session_status(session_id, OrderSessionStatus::Sent, user_id)
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
    /// Once the session is closed/sent, orders become immutable.
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

        // Case 2: Look for open sessions
        let open_sessions = self
            .repository
            .list_open_sessions_for_restaurant(restaurant_id)
            .await?;

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
        let today = now.date_naive();

        // Build start and end DateTimes for today
        let start_dt = today
            .and_time(settings.default_start_time)
            .and_utc();
        let end_dt = today
            .and_time(settings.default_end_time)
            .and_utc();

        // If the end time has already passed, schedule for tomorrow
        let (start_dt, end_dt) = if end_dt <= now {
            let tomorrow = today.succ_opt().ok_or_else(|| anyhow!("Date overflow"))?;
            (
                tomorrow.and_time(settings.default_start_time).and_utc(),
                tomorrow.and_time(settings.default_end_time).and_utc(),
            )
        } else {
            (start_dt, end_dt)
        };

        let create_request = CreateOrderSession {
            restaurant_id,
            start_date: start_dt,
            end_date: end_dt,
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