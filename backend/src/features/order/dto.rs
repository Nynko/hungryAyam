use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::features::order::domain::{
    order::CreateOrder,
    order_session::{CreateOrderSession, OrderSession, UpdateOrderSession},
    order_settings::{CreateRestaurantOrderSettings, SendingMethod},
};

// ==================== Order Session DTOs ====================

pub type CreateOrderSessionRequest = CreateOrderSession;
pub type UpdateOrderSessionRequest = UpdateOrderSession;

/// Response returned after a session status transition (cancel, close, send).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderSessionStatusResponse {
    pub session: OrderSession,
}

// ==================== Order DTOs ====================

pub type CreateOrderRequest = CreateOrder;

/// Summary of an order for list views (without full item details).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderSummary {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub total_price_cents: i32,
    pub item_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ==================== Order Settings DTOs ====================

pub type CreateOrderSettingsRequest = CreateRestaurantOrderSettings;

/// Custom update DTO for restaurant order settings.
///
/// We need a hand-written struct instead of the macro-generated
/// `UpdateRestaurantOrderSettings` because `menu_reset_time` is
/// `Option<NaiveTime>` in the domain — an `update(all_optional)` macro
/// keeps it as `Option<NaiveTime>` (no double-wrap), so we can't
/// distinguish "don't change" from "set to null".
///
/// The `update_menu_reset_time` flag solves this:
/// - `false` → leave the current value untouched
/// - `true`  → write `menu_reset_time` (which may be `null`)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateOrderSettingsRequest {
    pub id: Uuid,
    #[ts(type = "string | null")]
    pub default_start_time: Option<NaiveTime>,
    #[ts(type = "string | null")]
    pub default_end_time: Option<NaiveTime>,
    pub sending_method: Option<SendingMethod>,
    pub timezone: Option<String>,
    pub auto_create_session: Option<bool>,
    /// The new value for `menu_reset_time`. Only written when
    /// `update_menu_reset_time` is `true`.
    #[ts(type = "string | null")]
    pub menu_reset_time: Option<NaiveTime>,
    /// When `true`, the `menu_reset_time` field is applied (even if `null`,
    /// which clears the reset schedule). When `false` (or absent), the
    /// existing `menu_reset_time` is left unchanged.
    #[serde(default)]
    pub update_menu_reset_time: bool,
    pub auto_close_session: Option<bool>,
}