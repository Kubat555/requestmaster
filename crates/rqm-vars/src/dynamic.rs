//! Dynamic values behind `{{$name}}` tokens: identifiers, timestamps, random numbers.

use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

/// Names [`system_dynamic`] knows, without the `$`.
pub const DYNAMIC_NAMES: [&str; 5] = ["guid", "randomUUID", "timestamp", "isoTimestamp", "randomInt"];

/// `guid`, `randomUUID` (UUID v4), `timestamp` (Unix seconds), `isoTimestamp` (UTC, `YYYY-MM-DDTHH:MM:SS.mmmZ`),
/// `randomInt` (0..=1000). Unknown name → `None`.
pub fn system_dynamic(name: &str) -> Option<String> {
    match name {
        "guid" | "randomUUID" => Some(Uuid::new_v4().to_string()),
        "timestamp" => Some(since_epoch().as_secs().to_string()),
        "isoTimestamp" => {
            let now = since_epoch();
            Some(iso_timestamp(now.as_secs(), now.subsec_millis()))
        }
        "randomInt" => {
            // The low 64 bits of a UUID v4 hold 62 random bits (the rest is the variant marker);
            // that is the random source already in the dependency tree.
            let random = Uuid::new_v4().as_u64_pair().1;
            Some((random % 1001).to_string())
        }
        _ => None,
    }
}

/// Time since the Unix epoch by the system clock; zero if the clock is set before 1970.
fn since_epoch() -> std::time::Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()
}

/// Formats a moment given as Unix seconds plus milliseconds as `YYYY-MM-DDTHH:MM:SS.mmmZ` (UTC).
fn iso_timestamp(unix_secs: u64, millis: u32) -> String {
    let (year, month, day) = civil_from_days(unix_secs / 86_400);
    let second_of_day = unix_secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60
    )
}

/// Gregorian `(year, month, day)` of the day number counted from 1970-01-01
/// (the "civil from days" algorithm by Howard Hinnant, for dates not before the epoch).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468; // days since 0000-03-01
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // from March 1
    let month_index = (5 * day_of_year + 2) / 153; // 0 = March
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_timestamp_of_known_dates() {
        assert_eq!(iso_timestamp(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_timestamp(946_684_799, 999), "1999-12-31T23:59:59.999Z");
        assert_eq!(iso_timestamp(951_782_400, 7), "2000-02-29T00:00:00.007Z");
        assert_eq!(iso_timestamp(1_709_209_845, 120), "2024-02-29T12:30:45.120Z");
        assert_eq!(iso_timestamp(1_709_251_200, 0), "2024-03-01T00:00:00.000Z");
    }
}
