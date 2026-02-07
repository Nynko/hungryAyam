use hungry_ayam_derive::IntoDomain;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::{
    features::{restaurant::domain::Restaurant},
    types::url::UrlString,
    traits::domain_traits::IntoDomain
};
use uuid::Uuid;


#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow,IntoDomain)]
#[into_domain(Restaurant)]
pub struct RestaurantRow {
    pub id: Uuid,
    pub name: String,
    #[domain_with_urlstring]
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
