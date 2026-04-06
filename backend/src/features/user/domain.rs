use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::types::{auth::AuthMethod, email::Email, name::Name, role::UserRole};

#[domain_struct(create, update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct User {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    pub name: Name,
    pub email: Option<Email>,
    pub auth_method: AuthMethod,
    #[serde(skip)]
    #[ts(skip)]
    #[derived_domain_ignore]
    pub password_hash: Option<String>,
    pub role: Option<UserRole>,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
}