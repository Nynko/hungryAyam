use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::{email::Email, name::Name, url::UrlString};

/// Request body for initial app setup.
///
/// Creates the app settings AND the first admin user in one step.
/// This is the only way to create an admin without already being an admin
/// (the "bootstrap" problem).
///
/// The `access_code` is the shared site password (memorable, human-readable).
/// It will be hashed with SHA-256 before storage — the plaintext is never persisted.
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppSetupRequest {
    // ── App settings ──────────────────────────────────────────────
    pub app_name: String,
    pub image_url: Option<UrlString>,
    /// Shared site password — users must enter this to access the site.
    /// Stored as SHA-256 hash, never in plaintext.
    pub access_code: String,

    // ── First admin user ──────────────────────────────────────────
    pub admin_name: Name,
    pub admin_email: Email,
    pub admin_password: String,
}