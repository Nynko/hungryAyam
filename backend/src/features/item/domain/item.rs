use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use ts_rs::TS;
use hungry_ayam_derive::domain_struct;
use crate::types::{
    url::UrlString,
    price::PriceCents,
    name::Name
};

use super::tag::{Tag, TagInput};

/// Item domain struct - represents a product/dish that can be sold
/// Tags are part of the domain model (fetched from junction table)
#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Item {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    #[update_ignore]
    pub restaurant_id: Uuid,
    pub name: Name,
    pub description: Option<String>,
    pub base_price_cents: PriceCents,
    pub image_url: Option<UrlString>,
    #[create_ignore]
    pub active: bool,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
    #[update_ignore]
    pub created_by: Uuid,
    #[create_ignore]
    pub updated_by: Uuid,

    /// Tags attached to this item
    /// Domain uses Vec<Tag> (full objects), Create/Update use Vec<TagInput>
    #[serde(default)] /// For TS : Create default if not present
    #[derived_type(Vec<TagInput>)]
    pub tags: Vec<Tag>,
}
