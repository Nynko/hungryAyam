use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::types::{auth::AuthMethod, email::Email, name::Name};

#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct User {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    pub name: Name,
    pub email: Option<Email>,
    pub auth_method: AuthMethod,
    pub auth_value: Option<String>,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn validate_auth_method(auth_method: &Option<String>) -> Result<(), anyhow::Error> {
        // if let Some(method) = auth_method {
        //     let valid_methods = ["password", "oauth", "google", "github", "guest"];
        //     if !valid_methods.contains(&method.as_str()) {
        //         anyhow::bail!(
        //             "Invalid auth method. Must be one of: {}",
        //             valid_methods.join(", ")
        //         );
        //     }
        // }

        // Ok(())
        todo!("Create AuthMethod struct or Enum and manage the type in the IntoDomain macro")
    }
}
