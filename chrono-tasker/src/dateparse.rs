//! Turn human-friendly text into a [`NaiveDate`].
//!
//! Supported forms (case-insensitive):
//!
//! | Input        | Meaning                                  |
//! |--------------|------------------------------------------|
//! | `today`      | the current day                          |
//! | `tomorrow`   | the next day                             |
//! | `+3d`        | three days from today                    |
//! | `+2w`        | two weeks from today                     |
//! | `+1m`        | one calendar month from today            |
//! | `fri`        | the next Friday strictly after today     |
//! | `2026-12-25` | an exact ISO date                        |

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

use crate::error::{Result, TaskError};

/// Parse `input` relative to `today`.
///
/// `today` is passed in (instead of calling the clock here) so the function
/// is pure and easy to test.
///
/// ```
/// use chrono::NaiveDate;
/// use chrono_tasker::dateparse::parse_due;
///
/// let today = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap(); // a Wednesday
/// assert_eq!(parse_due("+3d", today).unwrap().to_string(), "2026-11-14");
/// assert_eq!(parse_due("fri", today).unwrap().to_string(), "2026-11-13");
/// assert!(parse_due("someday", today).is_err());
/// ```
pub fn parse_due(input: &str, today: NaiveDate) -> Result<NaiveDate> {
    let raw = input.trim().to_ascii_lowercase();
    let invalid = || TaskError::InvalidDate(input.trim().to_string());

    match raw.as_str() {
        "today" => return Ok(today),
        "tomorrow" => return today.checked_add_days(Days::new(1)).ok_or_else(invalid),
        _ => {}
    }

    if let Some(offset) = raw.strip_prefix('+') {
        return parse_offset(offset, today).ok_or_else(invalid);
    }

    if let Ok(date) = NaiveDate::parse_from_str(&raw, "%Y-%m-%d") {
        return Ok(date);
    }

    if let Ok(weekday) = raw.parse::<Weekday>() {
        return Ok(next_weekday(today, weekday));
    }

    Err(invalid())
}

/// Parse the part after `+`, e.g. `3d`, `2w` or `1m`.
fn parse_offset(offset: &str, today: NaiveDate) -> Option<NaiveDate> {
    let unit = offset.chars().last()?;
    let amount: u32 = offset[..offset.len() - unit.len_utf8()].parse().ok()?;

    match unit {
        'd' => today.checked_add_days(Days::new(u64::from(amount))),
        'w' => today.checked_add_days(Days::new(u64::from(amount) * 7)),
        'm' => today.checked_add_months(Months::new(amount)),
        _ => None,
    }
}

/// The first `target` weekday strictly after `from` (never `from` itself).
fn next_weekday(from: NaiveDate, target: Weekday) -> NaiveDate {
    let from_index = from.weekday().num_days_from_monday();
    let target_index = target.num_days_from_monday();
    let ahead = match (target_index + 7 - from_index) % 7 {
        0 => 7,
        days => days,
    };
    from.checked_add_days(Days::new(u64::from(ahead)))
        .unwrap_or(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Wednesday.
    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn keywords() {
        assert_eq!(parse_due("today", today()).unwrap(), today());
        assert_eq!(
            parse_due("  Tomorrow ", today()).unwrap(),
            date(2026, 11, 12)
        );
    }

    #[test]
    fn offsets() {
        assert_eq!(parse_due("+3d", today()).unwrap(), date(2026, 11, 14));
        assert_eq!(parse_due("+2w", today()).unwrap(), date(2026, 11, 25));
        assert_eq!(parse_due("+1m", today()).unwrap(), date(2026, 12, 11));
        assert_eq!(parse_due("+0d", today()).unwrap(), today());
    }

    #[test]
    fn exact_dates() {
        assert_eq!(
            parse_due("2026-12-25", today()).unwrap(),
            date(2026, 12, 25)
        );
        assert!(parse_due("2026-02-30", today()).is_err());
    }

    #[test]
    fn weekdays_are_strictly_in_the_future() {
        assert_eq!(parse_due("fri", today()).unwrap(), date(2026, 11, 13));
        assert_eq!(parse_due("Monday", today()).unwrap(), date(2026, 11, 16));
        // Today is Wednesday, so "wed" means next week.
        assert_eq!(parse_due("wed", today()).unwrap(), date(2026, 11, 18));
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "banana", "+", "+d", "+3", "+3x", "+-1d", "11/11/2026"] {
            assert!(
                matches!(parse_due(bad, today()), Err(TaskError::InvalidDate(_))),
                "{bad:?} should be rejected"
            );
        }
    }
}
