use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::features::order::domain::{
    order::CreateOrder,
    order_session::{CreateOrderSession, OrderSession},
    order_settings::{CreateRestaurantOrderSettings, SendingMethod},
};

// ==================== Order Session DTOs ====================

pub type CreateOrderSessionRequest = CreateOrderSession;

/// Hand-written update DTO for order sessions.
///
/// `pickup_time` uses the same boolean-flag pattern as `menu_reset_time` in
/// order settings, so the client can explicitly clear it (set to null) without
/// ambiguity with "don't change" (field absent / null in the patch payload).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateOrderSessionRequest {
    pub id: Uuid,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    /// When true, `pickup_time` is written (even if null, which clears it).
    /// When false, the existing value is left unchanged.
    #[serde(default)]
    pub update_pickup_time: bool,
    pub pickup_time: Option<DateTime<Utc>>,
    pub allow_late: Option<bool>,
}

/// Response returned after a session status transition (cancel, close, send).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderSessionStatusResponse {
    pub session: OrderSession,
}

// ==================== Order DTOs ====================

pub type CreateOrderRequest = CreateOrder;

/// Request body to move an order to a different session.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MoveOrderToSessionRequest {
    pub new_session_id: Uuid,
}

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

// ==================== Session Order Summary DTOs ====================

/// One regular (non-offer) item line in the aggregated summary.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RegularItemSummary {
    pub item_name: String,
    pub quantity: i64,
    pub note: Option<String>,
}

/// A single item/combo entry inside an offer slot.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OfferItemCount {
    pub name: String,
    pub qty: i64,
}

/// One slot row inside an offer group.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OfferSlotSummary {
    pub label: String,
    pub items: Vec<OfferItemCount>,
}

/// All orders for one offer in the aggregated summary.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OfferGroupSummary {
    pub offer_title: String,
    pub count: i64,
    pub slots: Vec<OfferSlotSummary>,
}

/// Full aggregated summary for a session, matching the frontend display logic.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionOrderSummary {
    pub regular_items: Vec<RegularItemSummary>,
    pub offer_groups: Vec<OfferGroupSummary>,
}

impl SessionOrderSummary {
    /// Render a compact plain-text representation suitable for SMS / email.
    pub fn to_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        for item in &self.regular_items {
            let line = match &item.note {
                Some(n) => format!("{}x {} ({})", item.quantity, item.item_name, n),
                None => format!("{}x {}", item.quantity, item.item_name),
            };
            parts.push(line);
        }

        for group in &self.offer_groups {
            parts.push(format!("{} x{}", group.offer_title, group.count));
            for slot in &group.slots {
                parts.push(format!("  {}:", slot.label));
                for item in &slot.items {
                    parts.push(format!("    {}x {}", item.qty, item.name));
                }
            }
        }

        parts.join("\n")
    }
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
    pub notify_on_session_close: Option<bool>,
    /// The new value for `default_pickup_time`. Only written when
    /// `update_default_pickup_time` is `true`.
    #[ts(type = "string | null")]
    pub default_pickup_time: Option<NaiveTime>,
    /// When `true`, `default_pickup_time` is applied (even if `null`, which clears it).
    #[serde(default)]
    pub update_default_pickup_time: bool,
}