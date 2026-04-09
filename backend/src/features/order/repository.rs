use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    features::order::{
        db_model::{OrderItemRow, OrderRow, OrderSessionRow, RestaurantOrderSettingsRow},
        domain::{
            order::{CreateOrderItem, Order, OrderItem},
            order_session::{
                CreateOrderSession, OrderSession, OrderSessionStatus,
            },
            order_settings::{
                CreateRestaurantOrderSettings, RestaurantOrderSettings,
            },
        },
        dto::{OrderSummary, UpdateOrderSessionRequest, UpdateOrderSettingsRequest},
    },
    types::price::PriceCents,
};

// SendingMethod must be in scope for sqlx::query_as! macro expansion
use crate::features::order::domain::order_settings::SendingMethod;

#[derive(Clone)]
pub struct OrderRepository {
    pool: PgPool,
}

impl OrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ==================== ORDER SESSION OPERATIONS ====================

    /// Create a new order session with status = Open.
    pub async fn create_session(
        &self,
        request: CreateOrderSession,
        user_id: Uuid,
    ) -> Result<OrderSession> {
        let status = OrderSessionStatus::Open;
        let row = sqlx::query_as!(
            OrderSessionRow,
            r#"
            INSERT INTO order_sessions (restaurant_id, start_date, end_date, pickup_time, allow_late, status, created_by, updated_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            "#,
            request.restaurant_id,
            request.start_date,
            request.end_date,
            request.pickup_time,
            request.allow_late,
            status.as_i16(),
            user_id,
            user_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(self.session_row_to_domain(row, vec![]))
    }

    /// Get a session by ID (without orders — use `get_session_with_orders` for the full view).
    pub async fn get_session_by_id(&self, id: Uuid) -> Result<Option<OrderSession>> {
        let row = sqlx::query_as!(
            OrderSessionRow,
            r#"
            SELECT
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM order_sessions
            WHERE id = $1
            "#,
            id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| self.session_row_to_domain(r, vec![])))
    }

    /// Get a session by ID with all its orders and order items.
    pub async fn get_session_with_orders(&self, id: Uuid) -> Result<Option<OrderSession>> {
        let session_row = sqlx::query_as!(
            OrderSessionRow,
            r#"
            SELECT
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM order_sessions
            WHERE id = $1
            "#,
            id,
        )
        .fetch_optional(&self.pool)
        .await?;

        match session_row {
            Some(row) => {
                let orders = self.load_orders_for_session(id).await?;
                Ok(Some(self.session_row_to_domain(row, orders)))
            }
            None => Ok(None),
        }
    }

    /// List all sessions for a restaurant, ordered by most recent first.
    pub async fn list_sessions_by_restaurant(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Vec<OrderSession>> {
        let rows = sqlx::query_as!(
            OrderSessionRow,
            r#"
            SELECT
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM order_sessions
            WHERE restaurant_id = $1
            ORDER BY created_at DESC
            "#,
            restaurant_id,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::with_capacity(rows.len());
        for r in rows {
            let orders = self.load_orders_for_session(r.id).await?;
            sessions.push(self.session_row_to_domain(r, orders));
        }
        Ok(sessions)
    }

    /// Get the currently active (Open) session for a restaurant, if any.
    pub async fn get_active_session_for_restaurant(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let row = sqlx::query_as!(
            OrderSessionRow,
            r#"
            SELECT
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM order_sessions
            WHERE restaurant_id = $1
              AND status = $2
              AND end_date > NOW()
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            restaurant_id,
            OrderSessionStatus::Open.as_i16(),
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let orders = self.load_orders_for_session(r.id).await?;
                Ok(Some(self.session_row_to_domain(r, orders)))
            }
            None => Ok(None),
        }
    }

    /// List all active (non-terminal) sessions for a restaurant, ordered by end_date ascending.
    /// Active = Open, Closed, Requested, Confirmed (excludes Cancelled and Finished).
    pub async fn list_open_sessions_for_restaurant(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Vec<OrderSession>> {
        let rows = sqlx::query_as!(
            OrderSessionRow,
            r#"
            SELECT
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM order_sessions
            WHERE restaurant_id = $1
              AND status = ANY(ARRAY[0, 1, 3, 4]::smallint[])
            ORDER BY end_date ASC
            "#,
            restaurant_id,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::with_capacity(rows.len());
        for r in rows {
            sessions.push(self.session_row_to_domain(r, vec![]));
        }
        Ok(sessions)
    }

    /// Count the number of orders in a session.
    pub async fn count_orders_in_session(&self, session_id: Uuid) -> Result<i64> {
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM orders WHERE session_id = $1"#,
            session_id,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// Move an order to a different session (updates session_id on the order).
    pub async fn move_order_to_session(
        &self,
        order_id: Uuid,
        new_session_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query!(
            "UPDATE orders SET session_id = $1 WHERE id = $2",
            new_session_id,
            order_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete a session unconditionally (used for auto-cleanup of empty sessions).
    /// Unlike `delete_session`, this does not check the status.
    pub async fn delete_session_unconditionally(&self, session_id: Uuid) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM order_sessions WHERE id = $1",
            session_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all orders placed by a specific user across all open sessions for a restaurant.
    pub async fn list_orders_by_user_in_open_sessions(
        &self,
        restaurant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Order>> {
        let order_rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                o.id,
                o.user_id,
                u.name as user_name,
                os.restaurant_id,
                o.session_id,
                o.offer_id,
                of.title as "offer_title?",
                o.total_price_cents as "total_price_cents: PriceCents",
                o.created_at
            FROM orders o
            JOIN order_sessions os ON os.id = o.session_id
            JOIN users u ON u.id = o.user_id
            LEFT JOIN offers of ON of.id = o.offer_id
            WHERE os.restaurant_id = $1
              AND o.user_id = $2
              AND os.status = $3
            ORDER BY os.end_date ASC, o.created_at ASC
            "#,
            restaurant_id,
            user_id,
            OrderSessionStatus::Open.as_i16(),
        )
        .fetch_all(&self.pool)
        .await?;

        self.order_rows_with_items(order_rows).await
    }

    /// Update mutable fields on an order session (start_date, end_date, allow_late).
    pub async fn update_session(
        &self,
        request: UpdateOrderSessionRequest,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let row = sqlx::query_as!(
            OrderSessionRow,
            r#"
            UPDATE order_sessions
            SET start_date  = COALESCE($1, start_date),
                end_date    = COALESCE($2, end_date),
                pickup_time = CASE WHEN $3::bool THEN $4 ELSE pickup_time END,
                allow_late  = COALESCE($5, allow_late),
                updated_at  = NOW(),
                updated_by  = $7
            WHERE id = $6
            RETURNING
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            "#,
            request.start_date,
            request.end_date,
            request.update_pickup_time,
            request.pickup_time,
            request.allow_late,
            request.id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| self.session_row_to_domain(r, vec![])))
    }

    /// Transition a session to a new status. Returns the updated session.
    pub async fn set_session_status(
        &self,
        session_id: Uuid,
        new_status: OrderSessionStatus,
        user_id: Uuid,
    ) -> Result<Option<OrderSession>> {
        let row = sqlx::query_as!(
            OrderSessionRow,
            r#"
            UPDATE order_sessions
            SET status     = $1,
                updated_at = NOW(),
                updated_by = $3
            WHERE id = $2
            RETURNING
                id,
                restaurant_id,
                start_date,
                end_date,
                pickup_time,
                allow_late,
                status as "status: OrderSessionStatus",
                created_at,
                created_by,
                updated_at,
                updated_by
            "#,
            new_status.as_i16(),
            session_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| self.session_row_to_domain(r, vec![])))
    }

    /// Delete a session (cascades to orders and order items).
    pub async fn delete_session(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM order_sessions WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ==================== ORDER OPERATIONS ====================

    /// Create an order with its items in a single transaction.
    /// `total_price_cents` must be pre-computed by the caller (service layer).
    /// `restaurant_id` is passed through from the service (not stored in orders table).
    pub async fn create_order(
        &self,
        restaurant_id: Uuid,
        session_id: Uuid,
        user_id: Uuid,
        total_price_cents: i32,
        offer_id: Option<Uuid>,
        items: &[CreateOrderItem],
    ) -> Result<Order> {
        let mut tx = self.pool.begin().await?;

        let order_row = sqlx::query_as!(
            OrderRow,
            r#"
            WITH inserted AS (
                INSERT INTO orders (user_id, session_id, offer_id, total_price_cents)
                VALUES ($1, $2, $3, $4)
                RETURNING *
            )
            SELECT
                i.id,
                i.user_id,
                u.name as user_name,
                os.restaurant_id,
                i.session_id,
                i.offer_id,
                of.title as "offer_title?",
                i.total_price_cents as "total_price_cents: PriceCents",
                i.created_at
            FROM inserted i
            JOIN order_sessions os ON os.id = i.session_id
            JOIN users u ON u.id = i.user_id
            LEFT JOIN offers of ON of.id = i.offer_id
            "#,
            user_id,
            session_id,
            offer_id,
            total_price_cents,
        )
        .fetch_one(&mut *tx)
        .await?;

        let mut order_items = Vec::with_capacity(items.len());
        for item in items {
            let item_row = sqlx::query_as!(
                OrderItemRow,
                r#"
                WITH inserted AS (
                    INSERT INTO order_items (order_id, item_id, slot_id, notes)
                    VALUES ($1, $2, $3, $4)
                    RETURNING id, order_id, item_id, slot_id, notes
                )
                SELECT
                    ins.id,
                    ins.order_id,
                    ins.item_id,
                    it.name as item_name,
                    it.base_price_cents as "item_price_cents: PriceCents",
                    ins.slot_id,
                    ofs.label as "slot_label?",
                    ins.notes
                FROM inserted ins
                JOIN items it ON it.id = ins.item_id
                LEFT JOIN offer_slots ofs ON ofs.id = ins.slot_id
                "#,
                order_row.id,
                item.item_id,
                item.slot_id,
                item.notes,
            )
            .fetch_one(&mut *tx)
            .await?;

            order_items.push(self.item_row_to_domain(item_row));
        }

        tx.commit().await?;

        Ok(self.order_row_to_domain(order_row, order_items))
    }

    /// Get an order by ID with its items.
    pub async fn get_order_by_id(&self, id: Uuid) -> Result<Option<Order>> {
        let order_row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                o.id,
                o.user_id,
                u.name as user_name,
                os.restaurant_id,
                o.session_id,
                o.offer_id,
                of.title as "offer_title?",
                o.total_price_cents as "total_price_cents: PriceCents",
                o.created_at
            FROM orders o
            JOIN order_sessions os ON os.id = o.session_id
            JOIN users u ON u.id = o.user_id
            LEFT JOIN offers of ON of.id = o.offer_id
            WHERE o.id = $1
            "#,
            id,
        )
        .fetch_optional(&self.pool)
        .await?;

        match order_row {
            Some(row) => {
                let items = self.load_items_for_order(row.id).await?;
                Ok(Some(self.order_row_to_domain(row, items)))
            }
            None => Ok(None),
        }
    }

    /// List all orders for a session (with items).
    pub async fn list_orders_by_session(&self, session_id: Uuid) -> Result<Vec<Order>> {
        self.load_orders_for_session(session_id).await
    }

    /// List orders for a specific user in a session (with items).
    pub async fn list_orders_by_user_in_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Order>> {
        let order_rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                o.id,
                o.user_id,
                u.name as user_name,
                os.restaurant_id,
                o.session_id,
                o.offer_id,
                of.title as "offer_title?",
                o.total_price_cents as "total_price_cents: PriceCents",
                o.created_at
            FROM orders o
            JOIN order_sessions os ON os.id = o.session_id
            JOIN users u ON u.id = o.user_id
            LEFT JOIN offers of ON of.id = o.offer_id
            WHERE o.session_id = $1 AND o.user_id = $2
            ORDER BY o.created_at ASC
            "#,
            session_id,
            user_id,
        )
        .fetch_all(&self.pool)
        .await?;

        self.order_rows_with_items(order_rows).await
    }

    /// Get order summaries for a session (lightweight list without full items).
    pub async fn list_order_summaries_by_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<OrderSummary>> {
        let summaries = sqlx::query_as!(
            OrderSummary,
            r#"
            SELECT
                o.id,
                o.user_id,
                o.session_id,
                o.total_price_cents,
                COUNT(oi.id) as "item_count!",
                o.created_at
            FROM orders o
            LEFT JOIN order_items oi ON oi.order_id = o.id
            WHERE o.session_id = $1
            GROUP BY o.id
            ORDER BY o.created_at ASC
            "#,
            session_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(summaries)
    }

    /// Delete an order (cascades to order items).
    pub async fn delete_order(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!("DELETE FROM orders WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ==================== ORDER SETTINGS OPERATIONS ====================

    /// Get order settings for a restaurant. Returns None if none have been configured.
    pub async fn get_settings_by_restaurant(
        &self,
        restaurant_id: Uuid,
    ) -> Result<Option<RestaurantOrderSettings>> {
        let settings = sqlx::query_as!(
            RestaurantOrderSettingsRow,
            r#"
            SELECT
                id,
                restaurant_id,
                default_start_time,
                default_end_time,
                sending_method as "sending_method: SendingMethod",
                timezone,
                auto_create_session,
                menu_reset_time,
                auto_close_session,
                created_at,
                updated_at
            FROM restaurant_order_settings
            WHERE restaurant_id = $1
            "#,
            restaurant_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(settings)
    }

    /// Get or create order settings for a restaurant (upsert with defaults).
    pub async fn get_or_create_settings(
        &self,
        restaurant_id: Uuid,
    ) -> Result<RestaurantOrderSettings> {
        let settings = sqlx::query_as!(
            RestaurantOrderSettingsRow,
            r#"
            INSERT INTO restaurant_order_settings (restaurant_id)
            VALUES ($1)
            ON CONFLICT (restaurant_id) DO UPDATE SET restaurant_id = EXCLUDED.restaurant_id
            RETURNING
                id,
                restaurant_id,
                default_start_time,
                default_end_time,
                sending_method as "sending_method: SendingMethod",
                timezone,
                auto_create_session,
                menu_reset_time,
                auto_close_session,
                created_at,
                updated_at
            "#,
            restaurant_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(settings)
    }

    /// Create order settings for a restaurant explicitly.
    pub async fn create_settings(
        &self,
        request: CreateRestaurantOrderSettings,
    ) -> Result<RestaurantOrderSettings> {
        let settings = sqlx::query_as!(
            RestaurantOrderSettingsRow,
            r#"
            INSERT INTO restaurant_order_settings (restaurant_id, default_start_time, default_end_time, sending_method, timezone, auto_create_session, menu_reset_time, auto_close_session)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                restaurant_id,
                default_start_time,
                default_end_time,
                sending_method as "sending_method: SendingMethod",
                timezone,
                auto_create_session,
                menu_reset_time,
                auto_close_session,
                created_at,
                updated_at
            "#,
            request.restaurant_id,
            request.default_start_time,
            request.default_end_time,
            request.sending_method.as_i16(),
            request.timezone,
            request.auto_create_session,
            request.menu_reset_time,
            request.auto_close_session,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(settings)
    }

    /// Update order settings for a restaurant.
    pub async fn update_settings(
        &self,
        request: UpdateOrderSettingsRequest,
    ) -> Result<Option<RestaurantOrderSettings>> {
        let settings = sqlx::query_as!(
            RestaurantOrderSettingsRow,
            r#"
            UPDATE restaurant_order_settings
            SET default_start_time  = COALESCE($1, default_start_time),
                default_end_time    = COALESCE($2, default_end_time),
                sending_method      = COALESCE($3, sending_method),
                timezone            = COALESCE($4, timezone),
                auto_create_session = COALESCE($5, auto_create_session),
                menu_reset_time     = CASE WHEN $7 THEN $6 ELSE menu_reset_time END,
                auto_close_session  = COALESCE($8, auto_close_session),
                updated_at          = NOW()
            WHERE id = $9
            RETURNING
                id,
                restaurant_id,
                default_start_time,
                default_end_time,
                sending_method as "sending_method: SendingMethod",
                timezone,
                auto_create_session,
                menu_reset_time,
                auto_close_session,
                created_at,
                updated_at
            "#,
            request.default_start_time,
            request.default_end_time,
            request.sending_method.map(|m| m.as_i16()),
            request.timezone,
            request.auto_create_session,
            request.menu_reset_time,
            request.update_menu_reset_time,
            request.auto_close_session,
            request.id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(settings)
    }

    // ==================== HELPERS: Load nested entities ====================

    /// Load all orders (with their items) for a given session.
    async fn load_orders_for_session(&self, session_id: Uuid) -> Result<Vec<Order>> {
        let order_rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                o.id,
                o.user_id,
                u.name as user_name,
                os.restaurant_id,
                o.session_id,
                o.offer_id,
                of.title as "offer_title?",
                o.total_price_cents as "total_price_cents: PriceCents",
                o.created_at
            FROM orders o
            JOIN order_sessions os ON os.id = o.session_id
            JOIN users u ON u.id = o.user_id
            LEFT JOIN offers of ON of.id = o.offer_id
            WHERE o.session_id = $1
            ORDER BY o.created_at ASC
            "#,
            session_id,
        )
        .fetch_all(&self.pool)
        .await?;

        self.order_rows_with_items(order_rows).await
    }

    /// Load all items for a given order.
    async fn load_items_for_order(&self, order_id: Uuid) -> Result<Vec<OrderItem>> {
        let item_rows = sqlx::query_as!(
            OrderItemRow,
            r#"
            SELECT
                oi.id,
                oi.order_id,
                oi.item_id,
                it.name as item_name,
                it.base_price_cents as "item_price_cents: PriceCents",
                oi.slot_id,
                ofs.label as "slot_label?",
                oi.notes
            FROM order_items oi
            JOIN items it ON it.id = oi.item_id
            LEFT JOIN offer_slots ofs ON ofs.id = oi.slot_id
            WHERE oi.order_id = $1
            ORDER BY COALESCE(ofs.position, 0) ASC, oi.id ASC
            "#,
            order_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(item_rows
            .into_iter()
            .map(|r| self.item_row_to_domain(r))
            .collect())
    }

    /// Batch-load items for multiple orders (single query, then group by order_id).
    async fn order_rows_with_items(&self, order_rows: Vec<OrderRow>) -> Result<Vec<Order>> {
        if order_rows.is_empty() {
            return Ok(vec![]);
        }

        let order_ids: Vec<Uuid> = order_rows.iter().map(|r| r.id).collect();

        let item_rows = sqlx::query_as!(
            OrderItemRow,
            r#"
            SELECT
                oi.id,
                oi.order_id,
                oi.item_id,
                it.name as item_name,
                it.base_price_cents as "item_price_cents: PriceCents",
                oi.slot_id,
                ofs.label as "slot_label?",
                oi.notes
            FROM order_items oi
            JOIN items it ON it.id = oi.item_id
            LEFT JOIN offer_slots ofs ON ofs.id = oi.slot_id
            WHERE oi.order_id = ANY($1)
            ORDER BY COALESCE(ofs.position, 0) ASC, oi.id ASC
            "#,
            &order_ids,
        )
        .fetch_all(&self.pool)
        .await?;

        // Group items by order_id
        let mut items_map: std::collections::HashMap<Uuid, Vec<OrderItem>> =
            std::collections::HashMap::new();
        for row in item_rows {
            let order_id = row.order_id;
            items_map
                .entry(order_id)
                .or_default()
                .push(self.item_row_to_domain(row));
        }

        Ok(order_rows
            .into_iter()
            .map(|row| {
                let items = items_map.remove(&row.id).unwrap_or_default();
                self.order_row_to_domain(row, items)
            })
            .collect())
    }

    // ==================== HELPERS: Price computation ====================

    /// Compute total price in cents for a set of items by looking up their base prices.
    pub async fn compute_total_price_cents(&self, item_ids: &[Uuid]) -> Result<i32> {
        let total: Option<i64> = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(base_price_cents), 0) as "total!"
            FROM items
            WHERE id = ANY($1)
            "#,
            item_ids,
        )
        .fetch_one(&self.pool)
        .await
        .map(Some)?;

        Ok(total.unwrap_or(0) as i32)
    }

    /// Check that all item IDs exist and belong to the given restaurant.
    /// Duplicates are allowed (e.g. ordering the same item twice) so we
    /// deduplicate before comparing with the DB count.
    pub async fn validate_item_ids(
        &self,
        item_ids: &[Uuid],
        restaurant_id: Uuid,
    ) -> Result<bool> {
        let unique_ids: Vec<Uuid> = {
            let mut set = std::collections::HashSet::new();
            item_ids.iter().filter(|id| set.insert(**id)).copied().collect()
        };

        let count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM items
            WHERE id = ANY($1) AND restaurant_id = $2
            "#,
            &unique_ids,
            restaurant_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count as usize == unique_ids.len())
    }

    // ==================== ROW → DOMAIN CONVERSIONS ====================

    fn session_row_to_domain(&self, row: OrderSessionRow, orders: Vec<Order>) -> OrderSession {
        OrderSession {
            id: row.id,
            restaurant_id: row.restaurant_id,
            start_date: row.start_date,
            end_date: row.end_date,
            pickup_time: row.pickup_time,
            allow_late: row.allow_late,
            status: row.status,
            created_at: row.created_at,
            created_by: row.created_by,
            updated_at: row.updated_at,
            updated_by: row.updated_by,
            orders,
        }
    }

    fn order_row_to_domain(&self, row: OrderRow, items: Vec<OrderItem>) -> Order {
        Order {
            id: row.id,
            user_id: row.user_id,
            user_name: row.user_name,
            restaurant_id: row.restaurant_id,
            session_id: Some(row.session_id),
            offer_id: row.offer_id,
            offer_title: row.offer_title,
            total_price_cents: row.total_price_cents,
            created_at: row.created_at,
            items,
        }
    }

    fn item_row_to_domain(&self, row: OrderItemRow) -> OrderItem {
        OrderItem {
            id: row.id,
            order_id: row.order_id,
            item_id: row.item_id,
            item_name: row.item_name,
            item_price_cents: row.item_price_cents,
            slot_id: row.slot_id,
            slot_label: row.slot_label,
            notes: row.notes,
        }
    }

}