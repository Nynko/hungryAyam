use chrono::{Datelike, NaiveDate};

/// Compute public holidays for a given country code (ISO 3166-1 alpha-2) and year.
/// Returns an empty vec for unsupported countries.
pub fn public_holidays(country: &str, year: i32) -> Vec<NaiveDate> {
    match country.to_uppercase().as_str() {
        "FR" => french_holidays(year),
        _ => vec![],
    }
}

/// Returns true if `date` is a public holiday in `country`.
pub fn is_public_holiday(country: &str, date: NaiveDate) -> bool {
    public_holidays(country, date.year()).contains(&date)
}

// ── France ────────────────────────────────────────────────────────

/// Compute French public holidays for a given year.
///
/// Fixed holidays:
///   1 Jan  — New Year's Day
///   1 May  — Labour Day
///   8 May  — Victory in Europe Day
///  14 Jul  — Bastille Day
///  15 Aug  — Assumption of Mary
///   1 Nov  — All Saints' Day
///  11 Nov  — Armistice Day
///  25 Dec  — Christmas Day
///
/// Easter-based (Gregorian):
///   Easter Sunday
///   Easter Monday  (+1)
///   Ascension Day  (+39)
///   Whit Monday    (+50)
fn french_holidays(year: i32) -> Vec<NaiveDate> {
    let easter = easter_sunday(year);

    let mut days = vec![
        // Fixed
        NaiveDate::from_ymd_opt(year, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(year, 5, 1).unwrap(),
        NaiveDate::from_ymd_opt(year, 5, 8).unwrap(),
        NaiveDate::from_ymd_opt(year, 7, 14).unwrap(),
        NaiveDate::from_ymd_opt(year, 8, 15).unwrap(),
        NaiveDate::from_ymd_opt(year, 11, 1).unwrap(),
        NaiveDate::from_ymd_opt(year, 11, 11).unwrap(),
        NaiveDate::from_ymd_opt(year, 12, 25).unwrap(),
        // Easter-based
        easter,
        easter + chrono::Duration::days(1),  // Easter Monday
        easter + chrono::Duration::days(39), // Ascension
        easter + chrono::Duration::days(50), // Whit Monday
    ];

    days.sort();
    days
}

/// Compute Easter Sunday for a given year using the Anonymous Gregorian algorithm.
fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easter_2024() {
        // Easter 2024 = March 31
        assert_eq!(easter_sunday(2024), NaiveDate::from_ymd_opt(2024, 3, 31).unwrap());
    }

    #[test]
    fn test_easter_2025() {
        // Easter 2025 = April 20
        assert_eq!(easter_sunday(2025), NaiveDate::from_ymd_opt(2025, 4, 20).unwrap());
    }

    #[test]
    fn test_french_holidays_2025_count() {
        let holidays = french_holidays(2025);
        assert_eq!(holidays.len(), 12);
    }

    #[test]
    fn test_french_fixed_holidays_2025() {
        let holidays = french_holidays(2025);
        assert!(holidays.contains(&NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()));
        assert!(holidays.contains(&NaiveDate::from_ymd_opt(2025, 5, 1).unwrap()));
        assert!(holidays.contains(&NaiveDate::from_ymd_opt(2025, 7, 14).unwrap()));
        assert!(holidays.contains(&NaiveDate::from_ymd_opt(2025, 12, 25).unwrap()));
    }

    #[test]
    fn test_french_easter_based_2025() {
        let holidays = french_holidays(2025);
        // Easter Monday 2025 = April 21
        assert!(holidays.contains(&NaiveDate::from_ymd_opt(2025, 4, 21).unwrap()));
        // Ascension 2025 = May 29 (April 20 + 39)
        assert!(holidays.contains(&NaiveDate::from_ymd_opt(2025, 5, 29).unwrap()));
        // Whit Monday 2025 = June 9 (April 20 + 50)
        assert!(holidays.contains(&NaiveDate::from_ymd_opt(2025, 6, 9).unwrap()));
    }

    #[test]
    fn test_is_public_holiday() {
        assert!(is_public_holiday("FR", NaiveDate::from_ymd_opt(2025, 7, 14).unwrap()));
        assert!(!is_public_holiday("FR", NaiveDate::from_ymd_opt(2025, 7, 15).unwrap()));
        assert!(!is_public_holiday("XX", NaiveDate::from_ymd_opt(2025, 7, 14).unwrap()));
    }
}
