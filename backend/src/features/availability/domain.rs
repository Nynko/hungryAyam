use chrono::{Datelike, NaiveDate, NaiveTime};
use hungry_ayam_derive::domain_struct;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::holidays::is_public_holiday;

/// Whether public holidays should be excluded from or required for availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PublicHolidaysMode {
    /// Entity is NOT available on public holidays.
    Exclude,
    /// Entity is ONLY available on public holidays.
    Only,
}

/// An availability rule defining when something (item, menu, or offer) is available.
///
/// All fields are optional constraints — if a field is `None`, that dimension is unrestricted:
/// - `valid_from` / `valid_to`: date range (inclusive)
/// - `start_time` / `end_time`: daily time window
/// - `weekdays`: which days of the week (0=Monday .. 6=Sunday, ISO 8601)
/// - `public_holidays_country` / `public_holidays_mode`: public holiday constraint
/// - `active`: master toggle (if false, the rule is ignored / entity treated as always-available)
#[domain_struct(create, update(all_optional))]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AvailabilityRule {
    #[create_ignore]
    #[update_required]
    pub id: Uuid,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    #[serde(default)]
    pub weekdays: Option<Vec<i16>>,
    /// ISO 3166-1 alpha-2 country code for public holiday computation (e.g. "FR").
    /// Must be set together with `public_holidays_mode`.
    #[serde(default)]
    pub public_holidays_country: Option<String>,
    /// Whether to exclude or require public holidays.
    /// Must be set together with `public_holidays_country`.
    #[serde(default)]
    pub public_holidays_mode: Option<PublicHolidaysMode>,
    /// Master toggle. When `false`, the rule is ignored and the entity is
    /// treated as always-available.
    #[update_required]
    pub active: bool,
}

impl AvailabilityRule {
    /// Check whether this rule allows availability at the given date and time.
    ///
    /// Returns `true` if all active constraints pass:
    /// - If `active` is false, returns `true` (rule disabled = always available)
    /// - If `valid_from` is set, date must be >= valid_from
    /// - If `valid_to` is set, date must be <= valid_to
    /// - If `weekdays` is set and non-empty, the weekday must be in the list
    /// - If `public_holidays_country` + `public_holidays_mode` are set, checks holiday constraint
    /// - If `start_time` and/or `end_time` are set, time must be in range
    ///   (supports overnight ranges like 22:00 - 06:00)
    pub fn is_available_at(&self, date: NaiveDate, time: NaiveTime) -> bool {
        if !self.active {
            return true; // Rule disabled — treat as always available
        }

        // Date range check
        if let Some(from) = self.valid_from {
            if date < from {
                return false;
            }
        }
        if let Some(to) = self.valid_to {
            if date > to {
                return false;
            }
        }

        // Weekday check (ISO 8601: Monday=0 .. Sunday=6)
        if let Some(ref days) = self.weekdays {
            if !days.is_empty() {
                let weekday_num = date.weekday().num_days_from_monday() as i16;
                if !days.contains(&weekday_num) {
                    return false;
                }
            }
        }

        // Public holiday check
        if let (Some(country), Some(mode)) = (&self.public_holidays_country, &self.public_holidays_mode) {
            let is_holiday = is_public_holiday(country, date);
            match mode {
                PublicHolidaysMode::Exclude => {
                    if is_holiday {
                        return false;
                    }
                }
                PublicHolidaysMode::Only => {
                    if !is_holiday {
                        return false;
                    }
                }
            }
        }

        // Time range check
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => {
                if start <= end {
                    // Normal range: e.g. 08:00 - 14:00
                    if time < start || time > end {
                        return false;
                    }
                } else {
                    // Overnight range: e.g. 22:00 - 06:00
                    if time < start && time > end {
                        return false;
                    }
                }
            }
            (Some(start), None) => {
                if time < start {
                    return false;
                }
            }
            (None, Some(end)) => {
                if time > end {
                    return false;
                }
            }
            (None, None) => {} // No time constraint
        }

        true
    }
}
