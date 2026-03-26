use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

/// AvailabilityRuleRow — maps directly to the `availability_rules` table.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct AvailabilityRuleRow {
    pub id: Uuid,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub weekdays: Option<Vec<i16>>,
    pub active: bool,
}