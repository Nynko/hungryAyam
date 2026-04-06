use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::features::item::domain::item::{CreateItem, Item, UpdateItem, UnitCreateItem};
use crate::types::price::PriceCents;

/// MenuSectionItem domain struct - links a catalog item to a menu section
/// Contains position and optional price override for menu-specific pricing
#[domain_struct(create, update(all_optional), unit_create)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MenuSectionItem {
    #[derived_domain_ignore]
    pub id: Uuid,
    #[create_ignore]
    #[unit_create_ignore]
    pub section_id: Uuid,
    // pub item_id: Uuid,
    pub position: i32,
    /// Optional price override - if None, use catalog item's base_price_cents
    pub price_override_cents: Option<PriceCents>,
    pub is_available: bool,
    // pub created_at: DateTime<Utc>,
    // pub updated_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub created_by: Uuid,
    #[derived_domain_ignore]
    pub updated_by: Uuid,
    #[derived_nested]
    pub item: Item,
}
