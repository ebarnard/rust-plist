use chrono::{DateTime, Duration, TimeZone, UTC};
use std::fmt;
use std::str::FromStr;

use {Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct Date {
    inner: DateTime<UTC>
}

impl Date {
    pub fn from_seconds_since_plist_epoch(timestamp: f64) -> Result<Date> {
        // Seconds since 1/1/2001 00:00:00.

        let millis = timestamp * 1_000.0;
        // Chrono's Duration can only millisecond values between ::std::i64::MIN and
        // ::std::i64::MAX.
        if millis > ::std::i64::MAX as f64 || millis < ::std::i64::MIN as f64 {
            return Err(Error::InvalidData);
        }

        let whole_millis = millis.floor();
        let submilli_nanos = ((millis - whole_millis) * 1_000_000.0).floor();

        let dur = Duration::milliseconds(whole_millis as i64);
        let dur = dur + Duration::nanoseconds(submilli_nanos as i64);

        let plist_epoch = UTC.ymd(2001, 1, 1).and_hms(0, 0, 0);
        let date = try!(plist_epoch.checked_add_signed(dur).ok_or(Error::InvalidData));

        Ok(Date {
            inner: date
        })
    }
}

impl From<DateTime<UTC>> for Date {
    fn from(date: DateTime<UTC>) -> Self {
        Date {
            inner: date
        }
    }
}

impl Into<DateTime<UTC>> for Date {
    fn into(self) -> DateTime<UTC> {
        self.inner
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl FromStr for Date {
    type Err = ();

    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        let date = DateTime::parse_from_rfc3339(&s).map_err(|_| ())?;
        Ok(Date {
            inner: date.with_timezone(&UTC)
        })
    }
}
