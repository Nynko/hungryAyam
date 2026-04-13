use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use ts_rs::TS;
use hungry_ayam_derive::domain_struct;
use crate::types::{
    url::ImageSource,
    name::Name
};
use crate::features::availability::domain::AvailabilityRule;

#[domain_struct(create, update(all_optional))]
// Remove derive(TS) and #[ts(export)] if the front end dto diverge from the domain
// With validated types implementing sqlx traits, we can derive FromRow directly!
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Restaurant {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    pub name: Name,
    pub image_url: Option<ImageSource>,
    pub phone_number: Option<String>,
    pub sms_phone_number: Option<String>,
    pub address: Option<String>,
    #[derived_domain_ignore]
    pub created_by: Uuid,
    #[derived_domain_ignore]
    pub updated_by: Uuid,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,

    /// Optional availability rule controlling when this restaurant is available.
    /// Populated on read; ignored on create/update (assigned via separate endpoint).
    #[serde(default)]
    #[derived_domain_ignore]
    pub availability_rule: Option<AvailabilityRule>,
}
