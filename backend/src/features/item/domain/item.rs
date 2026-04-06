use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use ts_rs::TS;
use hungry_ayam_derive::domain_struct;
use crate::types::{
    url::ImageSource,
    price::PriceCents,
    name::Name
};
use crate::features::availability::domain::AvailabilityRule;

use super::tag::Tag;

/// Item domain struct - represents a product/dish that can be sold
/// Tags are part of the domain model (fetched from junction table)
#[domain_struct(create, update(all_optional), unit_create)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Item {
    #[create_ignore]
    #[unit_create_ignore]
    #[update_required]
    pub id: Uuid,
    #[update_ignore]
    pub restaurant_id: Uuid,
    pub name: Name,
    pub description: Option<String>,
    pub base_price_cents: PriceCents,
    pub image_url: Option<ImageSource>,
    #[create_ignore]
    pub active: bool,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub created_by: Uuid,
    #[derived_domain_ignore]
    pub updated_by: Uuid,

    /// Tags attached to this item (read model — full objects fetched from DB).
    /// Excluded from all variants; handled explicitly in DTO layer via TagInput.
    #[serde(default)]
    #[derived_domain_ignore]
    pub tags: Vec<Tag>,

    /// Optional availability rule controlling when this item is available.
    /// Populated on read; ignored on create/update (assigned via separate endpoint).
    #[serde(default)]
    #[derived_domain_ignore]
    pub availability_rule: Option<AvailabilityRule>,
}