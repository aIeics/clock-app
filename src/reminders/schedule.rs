//! Pure date/time rules for reminders, kept out of the UI so they can be tested.

use chrono::{DateTime, Duration, Local, NaiveDateTime, NaiveTime};

use super::Repeat;

/// Parses `YYYY-MM-DD HH:MM` or `HH:MM`.
///
/// A time without a date means today; if that time has already passed it means
/// tomorrow. Returns `None` for ambiguous local times (a DST clock change).
pub fn parse_when(input: &str, now: &DateTime<Local>) -> Option<DateTime<Local>> {
    let text = input.trim();

    let naive = if let Ok(date_time) = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M") {
        date_time
    } else {
        let time = NaiveTime::parse_from_str(text, "%H:%M").ok()?;
        let mut date_time = now.date_naive().and_time(time);
        if date_time <= now.naive_local() {
            date_time += Duration::days(1);
        }
        date_time
    };

    naive.and_local_timezone(Local).single()
}

/// The next occurrence strictly after `now`. Non-repeating reminders return the
/// original time unchanged: `fire_due` removes them instead.
pub fn next_occurrence(
    when: DateTime<Local>,
    repeat: Repeat,
    now: DateTime<Local>,
) -> DateTime<Local> {
    let step = match repeat {
        Repeat::Once => return when,
        Repeat::Daily => Duration::days(1),
        Repeat::Weekly => Duration::days(7),
    };

    let mut next = when;
    while next <= now {
        next += step;
    }
    next
}

/// Short label used in the reminder list.
pub fn format_when(when: &DateTime<Local>) -> String {
    when.format("%a %-d %b, %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{LocalResult, NaiveDate, TimeZone};

    fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        let naive = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap();
        match Local.from_local_datetime(&naive) {
            LocalResult::Single(time) => time,
            other => panic!("test time is not unambiguous here: {other:?}"),
        }
    }

    #[test]
    fn parses_a_full_date_and_time() {
        let now = at(2026, 11, 24, 12, 0);
        let parsed = parse_when("2026-12-25 09:30", &now).expect("valid date");
        assert_eq!(
            parsed.format("%Y-%m-%d %H:%M").to_string(),
            "2026-12-25 09:30"
        );
    }

    #[test]
    fn a_time_only_means_today_or_tomorrow() {
        let now = at(2026, 11, 24, 12, 0);
        let later = parse_when("14:15", &now).expect("later today");
        assert_eq!(
            later.format("%Y-%m-%d %H:%M").to_string(),
            "2026-11-24 14:15"
        );

        let earlier = parse_when("09:15", &now).expect("tomorrow");
        assert_eq!(
            earlier.format("%Y-%m-%d %H:%M").to_string(),
            "2026-11-25 09:15"
        );
    }

    #[test]
    fn rejects_garbage_and_rounds_trimmed_input() {
        let now = at(2026, 11, 24, 12, 0);
        assert_eq!(parse_when("not a date", &now), None);
        assert_eq!(parse_when("", &now), None);
        assert!(parse_when("  14:15  ", &now).is_some());
    }

    #[test]
    fn daily_repeat_skips_every_missed_day() {
        let when = at(2026, 11, 20, 9, 0);
        let now = at(2026, 11, 24, 12, 0);
        let next = next_occurrence(when, Repeat::Daily, now);
        assert_eq!(
            next.format("%Y-%m-%d %H:%M").to_string(),
            "2026-11-25 09:00"
        );
    }

    #[test]
    fn weekly_repeat_skips_missed_weeks() {
        let when = at(2026, 11, 3, 9, 0);
        let now = at(2026, 11, 24, 12, 0);
        let next = next_occurrence(when, Repeat::Weekly, now);
        assert_eq!(
            next.format("%Y-%m-%d %H:%M").to_string(),
            "2026-12-01 09:00"
        );
    }

    #[test]
    fn a_future_time_stays_put() {
        let when = at(2026, 11, 25, 9, 0);
        let now = at(2026, 11, 24, 12, 0);
        assert_eq!(next_occurrence(when, Repeat::Daily, now), when);
        assert_eq!(next_occurrence(when, Repeat::Once, now), when);
    }

    #[test]
    fn formats_a_short_label() {
        assert_eq!(format_when(&at(2026, 11, 25, 9, 5)), "Wed 25 Nov, 09:05");
    }
}
