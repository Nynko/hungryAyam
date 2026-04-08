use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    features::order::domain::{
        order_session::OrderSessionStatus,
        order_settings::RestaurantOrderSettings,
    },
    types::price::PriceCents,
};

/// OrderSessionRow — maps directly to the `order_sessions` table.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct OrderSessionRow {
    pub id: Uuid,
    pub restaurant_id: Uuid,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub pickup_time: Option<DateTime<Utc>>,
    pub allow_late: bool,
    pub status: OrderSessionStatus,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Uuid,
}

/// OrderRow — maps to the `orders` table joined with `order_sessions` for `restaurant_id`.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct OrderRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub restaurant_id: Uuid,
    pub session_id: Uuid,
    pub offer_id: Option<Uuid>,
    pub total_price_cents: PriceCents,
    pub created_at: DateTime<Utc>,
}

/// OrderItemRow — maps directly to the `order_items` table.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct OrderItemRow {
    pub id: Uuid,
    pub order_id: Uuid,
    pub item_id: Uuid,
    pub item_name: String,
    pub item_price_cents: PriceCents,
    pub slot_id: Option<Uuid>,
    pub notes: Option<String>,
}

/// RestaurantOrderSettingsRow — the domain struct derives `FromRow` directly,
/// so this is just a type alias (same pattern as `RestaurantRow`).
pub type RestaurantOrderSettingsRow = RestaurantOrderSettings;