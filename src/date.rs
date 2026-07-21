use std::{
    fmt,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

/// A UTC timestamp used for serialization to and from the plist date type.
///
/// Note that while this type implements `Serialize` and `Deserialize` it will behave strangely if
/// used with serializers from outside this crate.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Date {
    inner: SystemTime,
}

/// An error indicating that a string was not a valid XML plist date.
#[derive(Debug)]
#[non_exhaustive]
pub struct InvalidXmlDate;

pub(crate) struct InfiniteOrNanDate;

impl Date {
    /// The unix timestamp of the plist epoch.
    const PLIST_EPOCH_UNIX_TIMESTAMP: Duration = Duration::from_secs(978_307_200);

    /// Converts an XML plist date string to a `Date`.
    pub fn from_xml_format(date: &str) -> Result<Self, InvalidXmlDate> {
        // RFC 3339 covers years 0000-9999, including UTC offsets and sub-second
        // precision. Dates outside that range use ISO-8601 expanded years, which
        // `time` cannot parse, so they are handled separately.
        if let Ok(offset) = OffsetDateTime::parse(date, &Rfc3339) {
            return Ok(Date {
                inner: offset.to_offset(UtcOffset::UTC).into(),
            });
        }
        from_expanded_xml_format(date)
    }

    /// Converts the `Date` to an XML plist date string.
    pub fn to_xml_format(&self) -> String {
        let seconds = unix_seconds(self.inner);
        let (year, month, day) = civil_from_days(seconds.div_euclid(SECONDS_PER_DAY));
        if (0..=9999).contains(&year) {
            // Within RFC 3339's range: keep the previous formatting exactly (this
            // also preserves any sub-second precision).
            let datetime: OffsetDateTime = self.inner.into();
            if let Ok(formatted) = datetime.format(&Rfc3339) {
                return formatted;
            }
        }
        // Apple's tools write ISO-8601 expanded years (e.g. `11476-…`, `-29719-…`)
        // for dates outside 0000-9999, where `time` panics instead. Format these
        // directly to match `plutil` byte for byte.
        let secs_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
        format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            secs_of_day / 3600,
            secs_of_day % 3600 / 60,
            secs_of_day % 60,
        )
    }

    pub(crate) fn from_seconds_since_plist_epoch(
        timestamp: f64,
    ) -> Result<Date, InfiniteOrNanDate> {
        // `timestamp` is the number of seconds since the plist epoch of 1/1/2001 00:00:00.
        let plist_epoch = UNIX_EPOCH + Date::PLIST_EPOCH_UNIX_TIMESTAMP;

        if !timestamp.is_finite() {
            return Err(InfiniteOrNanDate);
        }

        let is_negative = timestamp < 0.0;
        let timestamp = timestamp.abs();
        let seconds = timestamp.floor() as u64;
        let subsec_nanos = (timestamp.fract() * 1e9) as u32;

        let dur_since_plist_epoch = Duration::new(seconds, subsec_nanos);

        let inner = if is_negative {
            plist_epoch.checked_sub(dur_since_plist_epoch)
        } else {
            plist_epoch.checked_add(dur_since_plist_epoch)
        };

        let inner = inner.ok_or(InfiniteOrNanDate)?;

        Ok(Date { inner })
    }

    pub(crate) fn as_seconds_since_plist_epoch(&self) -> f64 {
        // needed until #![feature(duration_float)] is stabilized
        fn as_secs_f64(d: Duration) -> f64 {
            const NANOS_PER_SEC: f64 = 1_000_000_000.00;
            (d.as_secs() as f64) + f64::from(d.subsec_nanos()) / NANOS_PER_SEC
        }

        let plist_epoch = UNIX_EPOCH + Date::PLIST_EPOCH_UNIX_TIMESTAMP;
        match self.inner.duration_since(plist_epoch) {
            Ok(dur_since_plist_epoch) => as_secs_f64(dur_since_plist_epoch),
            Err(err) => -as_secs_f64(err.duration()),
        }
    }
}

const SECONDS_PER_DAY: i128 = 86_400;

/// Whole seconds between `time` and the Unix epoch. Sub-second precision is
/// dropped, matching Apple, which truncates XML dates to whole seconds.
fn unix_seconds(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => i128::from(duration.as_secs()),
        Err(err) => -i128::from(err.duration().as_secs()),
    }
}

/// Parses an ISO-8601 date with an expanded (and possibly negative) year, as
/// written by Apple's tools for dates outside 0000-9999.
fn from_expanded_xml_format(date: &str) -> Result<Date, InvalidXmlDate> {
    let date = date.strip_suffix('Z').ok_or(InvalidXmlDate)?;
    let (date, time) = date.split_once('T').ok_or(InvalidXmlDate)?;

    let (negative, digits) = match date.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, date.strip_prefix('+').unwrap_or(date)),
    };
    let mut ymd = digits.splitn(3, '-');
    let year: i64 = next_field(&mut ymd)?;
    let month: u8 = next_field(&mut ymd)?;
    let day: u8 = next_field(&mut ymd)?;
    let year = i128::from(if negative { -year } else { year });

    // Whole seconds only; ignore any fractional part.
    let time = time.split('.').next().unwrap_or(time);
    let mut hms = time.split(':');
    let hour: u8 = next_field(&mut hms)?;
    let minute: u8 = next_field(&mut hms)?;
    let second: u8 = next_field(&mut hms)?;
    if hms.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return Err(InvalidXmlDate);
    }

    let days = days_from_civil(year, month, day);
    // Reject impossible calendar dates (e.g. the 30th of February).
    if civil_from_days(days) != (year, month, day) {
        return Err(InvalidXmlDate);
    }
    let seconds = days * SECONDS_PER_DAY
        + i128::from(hour) * 3600
        + i128::from(minute) * 60
        + i128::from(second);
    date_from_unix_seconds(seconds)
}

fn next_field<'a, T, I>(fields: &mut I) -> Result<T, InvalidXmlDate>
where
    T: std::str::FromStr,
    I: Iterator<Item = &'a str>,
{
    fields
        .next()
        .ok_or(InvalidXmlDate)?
        .parse()
        .map_err(|_| InvalidXmlDate)
}

fn date_from_unix_seconds(seconds: i128) -> Result<Date, InvalidXmlDate> {
    let inner = if seconds >= 0 {
        u64::try_from(seconds)
            .ok()
            .and_then(|s| UNIX_EPOCH.checked_add(Duration::from_secs(s)))
    } else {
        u64::try_from(-seconds)
            .ok()
            .and_then(|s| UNIX_EPOCH.checked_sub(Duration::from_secs(s)))
    };
    inner.map(|inner| Date { inner }).ok_or(InvalidXmlDate)
}

/// Civil date (proleptic Gregorian, astronomical year numbering) from a count of
/// days since 1970-01-01. Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i128) -> (i128, u8, u8) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
    let year = yoe + era * 400 + i128::from(month <= 2);
    (year, month, day)
}

/// Inverse of [`civil_from_days`]. Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: i128, month: u8, day: u8) -> i128 {
    let month = i128::from(month);
    let year = year - i128::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let doy = (153 * if month > 2 { month - 3 } else { month + 9 } + 2) / 5 + i128::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl fmt::Debug for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.to_xml_format())
    }
}

impl From<SystemTime> for Date {
    fn from(date: SystemTime) -> Self {
        Date { inner: date }
    }
}

impl From<Date> for SystemTime {
    fn from(val: Date) -> Self {
        val.inner
    }
}

impl fmt::Display for InvalidXmlDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("String was not a valid XML plist date")
    }
}

impl std::error::Error for InvalidXmlDate {}

#[cfg(feature = "serde")]
pub mod serde_impls {
    use serde::{
        de::{Deserialize, Deserializer, Error, Unexpected, Visitor},
        ser::{Serialize, Serializer},
    };
    use std::fmt;

    use crate::Date;

    pub const DATE_NEWTYPE_STRUCT_NAME: &str = "PLIST-DATE";

    impl Serialize for Date {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let date_str = self.to_xml_format();
            serializer.serialize_newtype_struct(DATE_NEWTYPE_STRUCT_NAME, &date_str)
        }
    }

    struct DateNewtypeVisitor;

    impl<'de> Visitor<'de> for DateNewtypeVisitor {
        type Value = Date;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a plist date newtype")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            DateStrVisitor.visit_str(v)
        }

        fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(DateStrVisitor)
        }
    }

    struct DateStrVisitor;

    impl Visitor<'_> for DateStrVisitor {
        type Value = Date;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a plist date string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Date::from_xml_format(v).map_err(|_| E::invalid_value(Unexpected::Str(v), &self))
        }
    }

    impl<'de> Deserialize<'de> for Date {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_newtype_struct(DATE_NEWTYPE_STRUCT_NAME, DateNewtypeVisitor)
        }
    }
}

#[cfg(test)]
mod testing {
    use super::*;

    #[test]
    fn date_roundtrip() {
        let date_str = "1981-05-16T11:32:06Z";

        let date = Date::from_xml_format(date_str).expect("should parse");

        let generated_str = date.to_xml_format();

        assert_eq!(date_str, generated_str);
    }

    #[test]
    fn far_past_date() {
        let date_str = "1920-01-01T00:00:00Z";
        Date::from_xml_format(date_str).expect("should parse");
    }

    fn from_unix(seconds: i64) -> Date {
        let inner = if seconds >= 0 {
            UNIX_EPOCH + Duration::from_secs(seconds as u64)
        } else {
            UNIX_EPOCH - Duration::from_secs((-seconds) as u64)
        };
        Date { inner }
    }

    // (seconds since the Unix epoch, XML the value should read/write). The XML is
    // what Apple's `plutil` produces for the same binary date.
    const EXPANDED: &[(i64, &str)] = &[
        (253_402_300_800, "10000-01-01T00:00:00Z"),
        (300_000_000_000, "11476-08-15T05:20:00Z"),
        (1_000_000_000_000, "33658-09-27T01:46:40Z"),
        (9_000_000_000_000, "287168-08-24T16:00:00Z"),
        (-62_170_156_800, "-001-11-28T00:00:00Z"),
        (-65_000_000_000, "-090-03-27T04:26:40Z"),
        (-1_000_000_000_000, "-29719-04-05T22:13:20Z"),
        (-9_000_000_000_000, "-283229-05-10T08:00:00Z"),
    ];

    #[test]
    fn writes_expanded_years_like_plutil() {
        for &(seconds, expected) in EXPANDED {
            assert_eq!(
                from_unix(seconds).to_xml_format(),
                expected,
                "seconds={seconds}"
            );
        }
    }

    #[test]
    fn reads_expanded_years() {
        for &(seconds, xml) in EXPANDED {
            let parsed = Date::from_xml_format(xml).expect("should parse expanded year");
            assert_eq!(parsed, from_unix(seconds));
            assert_eq!(parsed.to_xml_format(), xml);
        }
    }

    #[test]
    fn year_boundary_does_not_panic() {
        // The last second RFC 3339 can represent, and the first it cannot.
        assert_eq!(
            from_unix(253_402_300_799).to_xml_format(),
            "9999-12-31T23:59:59Z"
        );
        assert_eq!(
            from_unix(253_402_300_800).to_xml_format(),
            "10000-01-01T00:00:00Z"
        );
        // Year 0001 stays on the RFC 3339 path.
        assert_eq!(
            from_unix(-62_135_596_800).to_xml_format(),
            "0001-01-01T00:00:00Z"
        );
    }

    #[test]
    fn debug_of_far_date_does_not_panic() {
        assert_eq!(
            format!("{:?}", from_unix(300_000_000_000)),
            "11476-08-15T05:20:00Z"
        );
    }

    #[test]
    fn rejects_invalid_expanded_dates() {
        for s in [
            "11476-02-30T00:00:00Z",                   // 30th of February
            "11476-13-01T00:00:00Z",                   // month 13
            "11476-08-15T25:00:00Z",                   // hour 25
            "11476-08-15T05:20:00",                    // no trailing Z
            "11476-08-15",                             // no time
            "abcd-08-15T05:20:00Z",                    // non-numeric year
            "99999999999999999999999-01-01T00:00:00Z", // year too large to parse
            "999999999999-01-01T00:00:00Z",            // year outside the representable range
        ] {
            assert!(Date::from_xml_format(s).is_err(), "should reject {s}");
        }
    }
}
