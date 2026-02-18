use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::{email::Email, name::Name, password::ClearPassword};

/// Request body for initial app setup.
///
/// Creates the app settings row and the first admin user.
/// App name and image are configured via environment variables at deploy time.
///
/// The `access_code` is the shared site password (memorable, human-readable).
/// It will be hashed with SHA-256 before storage — the plaintext is never persisted.
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSetupRequest {
    /// Shared site password — users must enter this to access the site.
    /// Stored as SHA-256 hash, never in plaintext.
    pub access_code: ClearPassword,

    /// Admin user's display name.
    pub admin_name: Name,

    /// Admin user's email address (used for login).
    pub admin_email: Email,

    /// Admin user's password (will be hashed before storage).
    pub admin_password: ClearPassword,
}