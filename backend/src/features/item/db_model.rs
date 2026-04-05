use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{
    name::Name,
    price::PriceCents,
    url::ImageSource,
};

/// ItemRow - Database row representation of an item
/// This maps directly to the `items` table (without tags which come from junction table)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ItemRow {
    pub id: Uuid,
    pub restaurant_id: Uuid,
    pub name: Name,
    pub description: Option<String>,
    pub base_price_cents: PriceCents,
    pub image_url: Option<ImageSource>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub availability_rule_id: Option<Uuid>,
}