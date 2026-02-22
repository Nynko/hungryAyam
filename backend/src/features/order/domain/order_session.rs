use chrono::{DateTime, Utc};
use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::order::Order;

// ==================== OrderSessionStatus Enum ====================

/// Status of an order session, stored as smallint in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum OrderSessionStatus {
    /// Session is open and accepting orders
    Open,
    /// Session is closed (time expired or manually closed), no more orders accepted
    Closed,
    /// Session was cancelled by an admin, all orders voided
    Cancelled,
    /// Orders from this session have been sent to the restaurant
    Sent,
}

impl OrderSessionStatus {
    pub fn as_i16(&self) -> i16 {
        match self {
            Self::Open => 0,
            Self::Closed => 1,
            Self::Cancelled => 2,
            Self::Sent => 3,
        }
    }

    pub fn from_i16(value: i16) -> anyhow::Result<Self> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Closed),
            2 => Ok(Self::Cancelled),
            3 => Ok(Self::Sent),
            _ => anyhow::bail!("Invalid OrderSessionStatus value: {}", value),
        }
    }

    /// Whether the session can still accept new orders
    pub fn is_accepting_orders(&self) -> bool {
        matches!(self, Self::Open)
    }

    /// Whether the session is in a terminal state (no further transitions)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Sent)
    }
}

impl std::fmt::Display for OrderSessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "Open"),
            Self::Closed => write!(f, "Closed"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Sent => write!(f, "Sent"),
        }
    }
}

// === SQLx Type: stored as SMALLINT (i16) in Postgres ===

impl sqlx::Type<sqlx::Postgres> for OrderSessionStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <i16 as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for OrderSessionStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let val = self.as_i16();
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&val, buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for OrderSessionStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'_>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let val = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(val).map_err(|e| e.to_string().into())
    }
}

// ==================== OrderSession Domain ====================

/// OrderSession domain struct — represents an ordering window for a restaurant.
///
/// A session has a start and end time. While the session is open, users can
/// place orders. An admin can close, cancel, or mark the session as sent.
#[domain_struct(create, update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderSession {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    #[update_ignore]
    pub restaurant_id: Uuid,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub allow_late: bool,
    /// Status is managed through explicit operations (cancel, close, send),
    /// not through generic updates.
    #[create_ignore]
    #[update_ignore]
    pub status: OrderSessionStatus,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub created_by: Uuid,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_by: Uuid,

    /// Orders within this session (populated when loading a full session)
    #[serde(default)]
    #[create_ignore]
    #[update_ignore]
    pub orders: Vec<Order>,
}