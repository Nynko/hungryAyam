use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{name::Name, price::PriceCents};

/// OfferRow — maps directly to the `offers` table.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct OfferRow {
    pub id: Uuid,
    pub restaurant_id: Uuid,
    pub menu_id: Option<Uuid>,
    pub title: Name,
    pub description: Option<String>,
    pub base_price_cents: PriceCents,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub availability_rule_id: Option<Uuid>,
}

/// OfferSlotRow — maps directly to the `offer_slots` table.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct OfferSlotRow {
    pub id: Uuid,
    pub offer_id: Uuid,
    pub label: Name,
    pub min_items: i32,
    pub max_items: i32,
    pub supplement_cents: i32,
    pub position: i32,
}

/// OfferSlotConstraintRow — maps directly to the `offer_slot_constraints` table.
/// Exactly one of `allowed_item_id`, `allowed_tag_id`, `allowed_section_id`
/// will be `Some` (enforced by the DB check constraint).
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct OfferSlotConstraintRow {
    pub id: Uuid,
    pub slot_id: Uuid,
    pub allowed_item_id: Option<Uuid>,
    pub allowed_tag_id: Option<Uuid>,
    pub allowed_section_id: Option<Uuid>,
    pub supplement_cents: i32,
}