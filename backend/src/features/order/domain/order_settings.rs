use chrono::{DateTime, NaiveTime, Utc};
use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

// ==================== SendingMethod Enum ====================

/// How orders from a session are dispatched to the restaurant.
/// Stored as smallint in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SendingMethod {
    /// Admin manually relays the order (e.g. phone call, walking over)
    Manual,
    /// System sends an SMS to the restaurant
    Sms,
    /// System sends a WhatsApp message to the restaurant
    WhatsApp,
    /// System sends an email to the restaurant
    Email,
}

impl SendingMethod {
    pub fn as_i16(&self) -> i16 {
        match self {
            Self::Manual => 0,
            Self::Sms => 1,
            Self::WhatsApp => 2,
            Self::Email => 3,
        }
    }

    pub fn from_i16(value: i16) -> anyhow::Result<Self> {
        match value {
            0 => Ok(Self::Manual),
            1 => Ok(Self::Sms),
            2 => Ok(Self::WhatsApp),
            3 => Ok(Self::Email),
            _ => anyhow::bail!("Invalid SendingMethod value: {}", value),
        }
    }
}

impl Default for SendingMethod {
    fn default() -> Self {
        Self::Manual
    }
}

impl std::fmt::Display for SendingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "Manual"),
            Self::Sms => write!(f, "Sms"),
            Self::WhatsApp => write!(f, "WhatsApp"),
            Self::Email => write!(f, "Email"),
        }
    }
}

// === SQLx Type: stored as SMALLINT (i16) in Postgres ===

impl sqlx::Type<sqlx::Postgres> for SendingMethod {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <i16 as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for SendingMethod {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let val = self.as_i16();
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&val, buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for SendingMethod {
    fn decode(
        value: sqlx::postgres::PgValueRef<'_>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let val = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(val).map_err(|e| e.to_string().into())
    }
}

// ==================== RestaurantOrderSettings Domain ====================

/// Per-restaurant configuration for order sessions.
///
/// Controls the default timing for new sessions (start/end times of day),
/// how orders are dispatched to the restaurant, and whether sessions are
/// created automatically when a user places the first order of the day.
///
/// There is a 1:1 relationship between a restaurant and its order settings.
/// Settings are created lazily (with defaults) the first time they are needed
/// and can be updated by an admin.
#[domain_struct(create, update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS, sqlx::FromRow)]
#[ts(export)]
pub struct RestaurantOrderSettings {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    #[update_ignore]
    pub restaurant_id: Uuid,
    /// Default start-of-day time for new sessions (e.g. 08:00)
    #[ts(type = "string")]
    pub default_start_time: NaiveTime,
    /// Default end-of-day time for new sessions (e.g. 11:00)
    #[ts(type = "string")]
    pub default_end_time: NaiveTime,
    /// How orders from completed sessions are dispatched to the restaurant
    pub sending_method: SendingMethod,
    /// IANA timezone name (e.g. "Asia/Jakarta", "Europe/Paris").
    /// The `default_start_time` and `default_end_time` are interpreted as
    /// local times in this timezone. Used to compute real UTC instants when
    /// auto-creating sessions or scheduling order dispatch.
    pub timezone: String,
    /// When true, a new session is created automatically when a user places an
    /// order and no active session exists for the restaurant.
    pub auto_create_session: bool,
    /// Time of day (in the restaurant's timezone) when non-permanent menus
    /// should be automatically reset (all items set to is_available = false).
    /// NULL means no automatic reset.
    #[ts(type = "string | null")]
    pub menu_reset_time: Option<NaiveTime>,
    /// When true, order sessions are automatically closed when their end_date
    /// passes. The scheduler transitions Open sessions whose end_date is in
    /// the past to Closed status.
    pub auto_close_session: bool,
    #[derived_domain_ignore]
    pub created_at: DateTime<Utc>,
    #[derived_domain_ignore]
    pub updated_at: DateTime<Utc>,
}