use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use ts_rs::TS;
use crate::types::url::UrlString;

// Remove derive(TS) and #[ts(export)] if the front end dto diverge from the domain
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub base_price_cents: u32,
    pub description: Option<String>,
    pub image_url: Option<UrlString>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub update_at: DateTime<Utc>,
    // pub tags : Vec<Tags>
}

impl Item {

    pub fn validate_name(name : &str) -> Result<(),anyhow::Error> {
        if name.trim().is_empty() {
            anyhow::bail!("Item name cannot be empty"); // Can be replaced by domain error later
        }

        if name.len() > 100 {
            anyhow::bail!("Item name cannot exceed 100 characters"); // Can be replaced by domain error later
        }

        Ok(())
    }

}
