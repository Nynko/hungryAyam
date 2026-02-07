use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use hungry_ayam_derive::IntoDomain;

use crate::{
    traits::domain_traits::IntoDomain,
    features::{item::domain::Item}, types::url::UrlString
};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, IntoDomain)]
#[into_domain(Item)]
pub struct ItemRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub base_price_cents: u32,
    #[domain_with_urlstring]
    pub image_url: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub update_at: DateTime<Utc>,
}
