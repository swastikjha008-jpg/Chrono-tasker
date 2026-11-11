//! Count down to a date using chrono's time zone types.
//!
//! The function is generic over any [`TimeZone`], so the CLI can pass
//! `Local` while tests use a fixed offset and stay deterministic.

use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, TimeZone};

/// Time from `now` until local midnight at the start of `target`.
///
/// Negative when `target` has already begun. Returns `None` if that
/// midnight does not exist in the zone (rare daylight-saving gaps).
///
/// ```
/// use chrono::{FixedOffset, NaiveDate, TimeDelta, TimeZone};
/// use chrono_tasker::countdown::time_until;
///
/// let ist = FixedOffset::east_opt(5 * 3600 + 1800).unwrap();
/// let now = ist.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap();
/// let target = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
/// assert_eq!(time_until(target, &now), Some(TimeDelta::hours(12)));
/// ```
pub fn time_until<Tz: TimeZone>(target: NaiveDate, now: &DateTime<Tz>) -> Option<TimeDelta> {
    let midnight = target
        .and_time(NaiveTime::MIN)
        .and_local_timezone(now.timezone())
        .earliest()?;
    Some(midnight.signed_duration_since(now))
}

/// Format a duration as `in 3d 4h 5m` or `3d 4h 5m ago`.
pub fn describe(delta: TimeDelta) -> String {
    let minutes = delta.num_minutes().abs();
    let (days, hours, mins) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    let body = format!("{days}d {hours}h {mins}m");

    if delta < TimeDelta::zero() {
        format!("{body} ago")
    } else {
        format!("in {body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    fn ist() -> FixedOffset {
        FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn counts_forward_in_a_fixed_zone() {
        let now = ist().with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap();
        let delta = time_until(date(2026, 11, 14), &now).unwrap();
        assert_eq!(delta, TimeDelta::days(2) + TimeDelta::hours(12));
        assert_eq!(describe(delta), "in 2d 12h 0m");
    }

    #[test]
    fn counts_backward() {
        let now = ist().with_ymd_and_hms(2026, 11, 11, 6, 30, 0).unwrap();
        let delta = time_until(date(2026, 11, 10), &now).unwrap();
        assert_eq!(delta, -(TimeDelta::hours(30) + TimeDelta::minutes(30)));
        assert_eq!(describe(delta), "1d 6h 30m ago");
    }

    #[test]
    fn zone_matters() {
        // 17:00 UTC is 22:30 in IST, so midnight arrives sooner there.
        let instant = Utc.with_ymd_and_hms(2026, 11, 11, 17, 0, 0).unwrap();
        let in_utc = time_until(date(2026, 11, 12), &instant).unwrap();
        let in_ist = time_until(date(2026, 11, 12), &instant.with_timezone(&ist())).unwrap();
        assert_eq!(in_utc, TimeDelta::hours(7));
        assert_eq!(in_ist, TimeDelta::minutes(90));
    }

    #[test]
    fn describe_zero_and_small_values() {
        assert_eq!(describe(TimeDelta::zero()), "in 0d 0h 0m");
        assert_eq!(describe(TimeDelta::minutes(59)), "in 0d 0h 59m");
        assert_eq!(describe(TimeDelta::minutes(-1)), "0d 0h 1m ago");
    }
}
