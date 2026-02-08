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

// Remove derive(TS) and #[ts(export)] if the front end dto diverge from the domain
#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Item {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
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
}
