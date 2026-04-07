use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{name::Name, url::ImageSource};

/// RestaurantRow — maps directly to the `restaurants` table.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct RestaurantRow {
    pub id: Uuid,
    pub name: Name,
    pub image_url: Option<ImageSource>,
    pub phone_number: Option<String>,
    pub address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Uuid,
    pub availability_rule_id: Option<Uuid>,
}