use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use ts_rs::TS;

// Remove the derive(sqlx:FromRow) if the database diverge from the domain
// Remove derive(TS) and #[ts(export)] if the front end dto diverge from the domain
#[derive(Debug, Clone, Serialize, Deserialize, TS, sqlx::FromRow)]
#[ts(export)]
pub struct AppSetup {
    pub id: i16,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}
