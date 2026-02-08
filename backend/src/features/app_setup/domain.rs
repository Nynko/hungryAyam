use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use ts_rs::TS;
use crate::types::url::UrlString;

// Remove derive(TS) and #[ts(export)] if the front end dto diverge from the domain
#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSetup {
    #[derived_domain_ignore]
    pub id: i16,
    pub title: String,
    pub image_url: Option<UrlString>,
    /// Maximum allowed nesting depth for menu sections (default: 2, max: 10)
    #[create_ignore]
    pub max_menu_nesting_depth: i16,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>
}