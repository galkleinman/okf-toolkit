//! Calendar dates and ISO 8601 timestamps.
//!
//! OKF needs only two things from dates: `stale_after` is compared against
//! today (§5.5), and `verified[].at` entries are ordered to find the most
//! recent (§5.2). Both are plain comparisons, so this is a small proleptic
//! Gregorian implementation rather than a datetime dependency.

/// A proleptic Gregorian calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    /// Parses `YYYY-MM-DD`, rejecting out-of-range and non-existent dates.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }

        let year: i32 = text[0..4].parse().ok()?;
        let month: u8 = parse_two_digits(&text[5..7])?;
        let day: u8 = parse_two_digits(&text[8..10])?;

        (month >= 1 && month <= 12 && day >= 1 && day <= days_in_month(year, month))
            .then_some(Self { year, month, day })
    }

    /// Converts a Unix timestamp in seconds to the UTC calendar date.
    pub fn from_unix_seconds(seconds: i64) -> Self {
        let days = seconds.div_euclid(86_400);
        Self::from_days_since_epoch(days)
    }

    /// Howard Hinnant's `days_from_civil`.
    fn to_days_since_epoch(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let month = i64::from(self.month);
        let shifted_month = if month > 2 { month - 3 } else { month + 9 };
        let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    /// Howard Hinnant's `civil_from_days`, shifted to a March-based year so
    /// the leap day lands at the end and month lengths follow a fixed pattern.
    fn from_days_since_epoch(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u8;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        } as u8;

        Self {
            year: (year + i64::from(month <= 2)) as i32,
            month,
            day,
        }
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn parse_two_digits(text: &str) -> Option<u8> {
    text.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| text.parse().ok())?
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        _ => 28,
    }
}

/// An ISO 8601 timestamp, ordered by date then time-of-day.
///
/// Only enough of the grammar is parsed to order two timestamps and recover
/// the date; offsets are normalised to UTC so mixed-offset bundles compare
/// correctly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    date: Date,
    /// Seconds since midnight UTC.
    seconds: i32,
}

impl Timestamp {
    pub fn parse(text: &str) -> Option<Self> {
        let (date_part, rest) = text.split_once(['T', ' '])?;
        let date = Date::parse(date_part)?;

        let (time_part, offset_seconds) = split_offset(rest)?;
        let mut fields = time_part.split(':');
        let hour: i32 = parse_two_digits(fields.next()?)?.into();
        let minute: i32 = parse_two_digits(fields.next()?)?.into();
        let second: i32 = match fields.next() {
            Some(field) => {
                // Fractional seconds do not affect ordering at our resolution,
                // but an empty or non-numeric fraction is still malformed.
                let (whole, fraction) = field.split_once('.').unwrap_or((field, "0"));
                if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                parse_two_digits(whole)?.into()
            }
            None => 0,
        };
        if fields.next().is_some() || hour > 23 || minute > 59 || second > 60 {
            return None;
        }

        // An offset can push the instant into the previous or next UTC day, so
        // the date is normalised alongside the time rather than kept as written.
        let total = hour * 3600 + minute * 60 + second - offset_seconds;
        let day_shift = i64::from(total).div_euclid(86_400);
        let seconds = i64::from(total).rem_euclid(86_400) as i32;
        let date = Date::from_days_since_epoch(date.to_days_since_epoch() + day_shift);
        Some(Self { date, seconds })
    }

    pub fn date(&self) -> Date {
        self.date
    }
}

/// Splits a time from its zone designator, returning the offset in seconds.
fn split_offset(rest: &str) -> Option<(&str, i32)> {
    if let Some(time) = rest.strip_suffix('Z').or_else(|| rest.strip_suffix('z')) {
        return Some((time, 0));
    }

    // A `-` or `+` after the hour position separates the offset. Times have no
    // sign of their own, so the last one is unambiguous.
    let Some(index) = rest.rfind(['+', '-']) else {
        return Some((rest, 0));
    };

    let (time, offset) = rest.split_at(index);
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let digits = &offset[1..];
    let (hours, minutes) = match digits.split_once(':') {
        Some((hours, minutes)) => (hours, minutes),
        None if digits.len() == 4 => digits.split_at(2),
        None => (digits, "00"),
    };

    let hours: i32 = parse_two_digits(hours)?.into();
    let minutes: i32 = parse_two_digits(minutes)?.into();
    (hours <= 23 && minutes <= 59).then_some((time, sign * (hours * 3600 + minutes * 60)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_dates() {
        assert_eq!(
            Date::parse("2026-08-09"),
            Some(Date {
                year: 2026,
                month: 8,
                day: 9
            })
        );
        assert_eq!(
            Date::parse("2024-02-29"),
            Some(Date {
                year: 2024,
                month: 2,
                day: 29
            })
        );
        assert_eq!(
            Date::parse("2000-02-29"),
            Some(Date {
                year: 2000,
                month: 2,
                day: 29
            })
        );
    }

    #[test]
    fn rejects_malformed_dates() {
        for bad in [
            "2026-8-09",
            "2026/08/09",
            "20260809",
            "2026-08-09T00:00:00Z",
            "",
            "abcd-ef-gh",
            "2026-08-9",
        ] {
            assert!(Date::parse(bad).is_none(), "{bad} should not parse");
        }
    }

    #[test]
    fn rejects_impossible_dates() {
        for bad in [
            "2026-13-01",
            "2026-00-01",
            "2026-01-00",
            "2026-01-32",
            "2026-04-31",
        ] {
            assert!(Date::parse(bad).is_none(), "{bad} should not parse");
        }
    }

    #[test]
    fn rejects_february_29_in_common_years() {
        assert!(Date::parse("2026-02-29").is_none());
        assert!(Date::parse("1900-02-29").is_none());
        assert!(Date::parse("2023-02-28").is_some());
    }

    #[test]
    fn dates_order_chronologically() {
        let earlier = Date::parse("2026-06-15").expect("valid");
        let later = Date::parse("2026-12-31").expect("valid");
        assert!(earlier < later);
        assert!(Date::parse("2025-12-31").expect("valid") < earlier);
    }

    #[test]
    fn dates_render_zero_padded() {
        assert_eq!(
            Date {
                year: 2026,
                month: 1,
                day: 5
            }
            .to_string(),
            "2026-01-05"
        );
    }

    #[test]
    fn converts_unix_seconds_to_dates() {
        assert_eq!(Date::from_unix_seconds(0).to_string(), "1970-01-01");
        assert_eq!(Date::from_unix_seconds(86_399).to_string(), "1970-01-01");
        assert_eq!(Date::from_unix_seconds(86_400).to_string(), "1970-01-02");
        assert_eq!(
            Date::from_unix_seconds(1_774_483_200).to_string(),
            "2026-03-26"
        );
        assert_eq!(
            Date::from_unix_seconds(951_782_400).to_string(),
            "2000-02-29"
        );
        assert_eq!(Date::from_unix_seconds(-86_400).to_string(), "1969-12-31");
    }

    #[test]
    fn day_number_round_trips_across_epochs_and_leap_years() {
        for text in [
            "1970-01-01",
            "1969-12-31",
            "2000-02-29",
            "2024-02-29",
            "1900-03-01",
            "2026-08-09",
            "1600-01-01",
            "2400-12-31",
        ] {
            let date = Date::parse(text).expect("valid");
            let days = date.to_days_since_epoch();
            assert_eq!(Date::from_days_since_epoch(days), date, "{text}");
        }
        assert_eq!(
            Date::parse("1970-01-01")
                .expect("valid")
                .to_days_since_epoch(),
            0
        );
        assert_eq!(
            Date::parse("1970-01-02")
                .expect("valid")
                .to_days_since_epoch(),
            1
        );
    }

    #[test]
    fn offsets_that_cross_midnight_carry_the_date() {
        let utc = Timestamp::parse("2026-05-28T22:53:05Z").expect("valid");
        assert_eq!(utc.date(), Date::parse("2026-05-28").expect("valid"));

        let next_day_local = Timestamp::parse("2026-05-29T00:53:05+02:00").expect("valid");
        assert_eq!(next_day_local, utc);
        assert_eq!(
            next_day_local.date(),
            Date::parse("2026-05-28").expect("valid")
        );

        let previous_day_local = Timestamp::parse("2026-05-28T23:30:00-02:00").expect("valid");
        assert_eq!(
            previous_day_local.date(),
            Date::parse("2026-05-29").expect("valid")
        );
    }

    #[test]
    fn parses_timestamps_with_a_zulu_offset() {
        let timestamp = Timestamp::parse("2026-06-25T09:00:00Z").expect("valid");
        assert_eq!(timestamp.date(), Date::parse("2026-06-25").expect("valid"));
    }

    #[test]
    fn timestamps_order_within_a_day() {
        let morning = Timestamp::parse("2026-06-25T09:00:00Z").expect("valid");
        let evening = Timestamp::parse("2026-06-25T21:30:00Z").expect("valid");
        assert!(morning < evening);
    }

    #[test]
    fn normalises_numeric_offsets_to_utc() {
        let utc = Timestamp::parse("2026-05-28T22:53:05Z").expect("valid");
        assert_eq!(
            Timestamp::parse("2026-05-28T22:53:05+00:00"),
            Some(utc.clone())
        );
        assert_eq!(
            Timestamp::parse("2026-05-29T00:53:05+02:00"),
            Some(utc.clone())
        );
        assert_eq!(
            Timestamp::parse("2026-05-28T20:53:05-0200"),
            Some(utc.clone())
        );
        assert_eq!(Timestamp::parse("2026-05-28T21:53:05-01"), Some(utc));
    }

    #[test]
    fn accepts_space_separator_and_fractional_seconds() {
        let with_t = Timestamp::parse("2026-06-25T09:00:00Z").expect("valid");
        assert_eq!(
            Timestamp::parse("2026-06-25 09:00:00Z"),
            Some(with_t.clone())
        );
        assert_eq!(Timestamp::parse("2026-06-25T09:00:00.123Z"), Some(with_t));
    }

    #[test]
    fn accepts_minute_precision_and_lowercase_zulu() {
        assert!(Timestamp::parse("2026-06-25T09:00z").is_some());
        assert!(Timestamp::parse("2026-06-25T09:00").is_some());
    }

    #[test]
    fn accepts_a_leap_second() {
        assert!(Timestamp::parse("2026-06-30T23:59:60Z").is_some());
    }

    #[test]
    fn rejects_malformed_timestamps() {
        for bad in [
            "2026-06-25",
            "not-a-date T 09:00:00Z",
            "2026-06-25T24:00:00Z",
            "2026-06-25T09:60:00Z",
            "2026-06-25T09:00:61Z",
            "2026-06-25T09:00:00:00Z",
            "2026-06-25T09:00:00+24:00",
            "2026-06-25T09:00:00+00:60",
            "2026-06-25T09:00:00+ab:00",
            "2026-06-25TXX:00:00Z",
            "2026-06-25T09:00:00.Z",
            "2026-06-25T09Z",
        ] {
            assert!(Timestamp::parse(bad).is_none(), "{bad} should not parse");
        }
    }

    #[test]
    fn offsets_can_cross_a_day_boundary_in_ordering() {
        let late = Timestamp::parse("2026-06-25T23:00:00Z").expect("valid");
        let early_next = Timestamp::parse("2026-06-26T01:00:00Z").expect("valid");
        assert!(late < early_next);
    }
}
