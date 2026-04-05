use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use ts_rs::TS;

use crate::types::password::HashedPassword;

// Remove derive(TS) and #[ts(export)] if the front end dto diverge from the domain
#[domain_struct(create, update)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSetup {
    #[derived_domain_ignore]
    pub id: i16,
    /// Maximum allowed nesting depth for menu sections (default: 2, max: 10)
    #[create_ignore]
    pub max_menu_nesting_depth: i16,
    /// SHA-256 hex hash of the shared site access code.
    /// The plaintext code is never stored — only this hash.
    pub access_hash: HashedPassword,
    /// Email domain that makes registered users eligible for Editor role.
    /// E.g. "example.com" — users with matching email can self-promote to Editor.
    #[create_ignore]
    pub editor_email_domain: Option<String>,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
}