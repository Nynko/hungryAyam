use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::features::order::domain::{
    order::CreateOrder,
    order_session::{CreateOrderSession, OrderSession, UpdateOrderSession},
    order_settings::{CreateRestaurantOrderSettings, UpdateRestaurantOrderSettings},
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
pub type UpdateOrderSettingsRequest = UpdateRestaurantOrderSettings;