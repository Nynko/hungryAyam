use chrono::{DateTime, Utc};
use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::types::price::PriceCents;

// ==================== OrderItem Domain ====================

/// An individual item within an order.
///
/// Each order item references an `Item` from the menu. The optional `slot_id`
/// links to an offer slot (for future offer support). Notes allow the user to
/// specify customisations (e.g. "no onions").
#[domain_struct(create)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderItem {
    #[create_ignore]
    pub id: Uuid,
    /// Assigned server-side when the parent order is created.
    #[create_ignore]
    pub order_id: Uuid,
    pub item_id: Uuid,
    /// Offer slot reference — reserved for future offer support, currently always None.
    #[create_ignore]
    pub slot_id: Option<Uuid>,
    pub notes: Option<String>,
}

// ==================== Order Domain ====================

/// An order placed by a user within an order session.
///
/// The `total_price_cents` is computed server-side from the constituent items
/// (and, in the future, any applicable offer pricing). The `offer_id` is
/// reserved for future offer support and is currently always None.
///
/// `restaurant_id` is not stored in the `orders` table but is loaded via a
/// JOIN with `order_sessions` for convenience. At creation time, it is required
/// so the service can resolve or auto-create a session.
///
/// `session_id` is always `Some` when loaded from the database. At creation
/// time it is optional: when `None`, the service resolves or auto-creates a
/// session based on `restaurant_id`.
#[domain_struct(create)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Order {
    #[create_ignore]
    pub id: Uuid,
    /// Assigned from the authenticated user; not provided by the client.
    #[create_ignore]
    pub user_id: Uuid,
    /// The restaurant this order belongs to.
    /// Loaded from the parent session; used at creation time to resolve the session.
    pub restaurant_id: Uuid,
    /// The session this order belongs to.
    /// Always `Some` when loaded from DB. Optional at creation time — when
    /// `None`, the service resolves or auto-creates a session.
    pub session_id: Option<Uuid>,
    /// Offer reference — reserved for future offer support, currently always None.
    #[create_ignore]
    pub offer_id: Option<Uuid>,
    /// Computed server-side from the order items' prices.
    #[create_ignore]
    pub total_price_cents: PriceCents,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,

    /// Items in this order (populated when loading a full order).
    /// At creation time, maps to `Vec<CreateOrderItem>`.
    #[serde(default)]
    #[derived_nested]
    pub items: Vec<OrderItem>,
}