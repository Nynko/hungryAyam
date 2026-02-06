use hungry_ayam_derive::IntoDomain;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::{
    types::url::UrlString,
    traits::domain_traits::IntoDomain,
    features::app_setup::domain::AppSetup
};


#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, IntoDomain)]
#[into_domain(AppSetup)]
pub struct AppSetupRow {
    pub id: i16,
    pub title: String,
    #[domain_with_urlstring]
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}
