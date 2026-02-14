use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::types::{email::Email, name::Name};

#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    pub name: Option<Name>,
    pub email: Option<Email>,
    pub auth_method: Option<String>,
    pub user_cookie: Option<String>,
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
