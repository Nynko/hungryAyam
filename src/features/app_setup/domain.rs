use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use ts_rs::TS;
use crate::types::url::UrlString;

// Remove derive(TS) and #[ts(export)] if the front end dto diverge from the domain
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSetup {
    pub id: i16,
    pub title: String,
    pub image_url: Option<UrlString>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}
