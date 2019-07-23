use std::{fmt, num::ParseIntError, str::FromStr};

/// An integer that can be represented by either an `i64` or a `u64`.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Integer {
    value: i128,
}

impl Integer {
    /// Returns the value as an `i64` if it can be represented by that type.
    pub fn as_signed(self) -> Option<i64> {
        if self.value >= i128::from(i64::min_value()) && self.value <= i128::from(i64::max_value())
        {
            Some(self.value as i64)
        } else {
            None
        }
    }

    /// Returns the value as a `u64` if it can be represented by that type.
    pub fn as_unsigned(self) -> Option<u64> {
        if self.value >= 0 && self.value <= i128::from(u64::max_value()) {
            Some(self.value as u64)
        } else {
            None
        }
    }
}

impl fmt::Debug for Integer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl FromStr for Integer {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("0x") {
            // NetBSD dialect adds the `0x` numeric objects,
            // which are always unsigned.
            // See the `PROP_NUMBER(3)` man page
            let s = s.trim_start_matches("0x");
            u64::from_str_radix(s, 16).map(Into::into)
        } else {
            // Match Apple's implementation in CFPropertyList.h - always try to parse as an i64 first.
            // TODO: Use IntErrorKind once stable and retry parsing on overflow only.
            Ok(match s.parse::<i64>() {
                Ok(v) => v.into(),
                Err(_) => s.parse::<u64>()?.into(),
            })
        }
    }
}

impl From<i64> for Integer {
    fn from(value: i64) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

impl From<i32> for Integer {
    fn from(value: i32) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

impl From<i16> for Integer {
    fn from(value: i16) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

impl From<i8> for Integer {
    fn from(value: i8) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

impl From<u64> for Integer {
    fn from(value: u64) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

impl From<u32> for Integer {
    fn from(value: u32) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

impl From<u16> for Integer {
    fn from(value: u16) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

impl From<u8> for Integer {
    fn from(value: u8) -> Integer {
        Integer {
            value: value.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Integer;

    #[test]
    fn from_str_limits() {
        assert_eq!("-1".parse::<Integer>(), Ok((-1).into()));
        assert_eq!("0".parse::<Integer>(), Ok(0.into()));
        assert_eq!("1".parse::<Integer>(), Ok(1.into()));
        assert_eq!(
            "-9223372036854775808".parse::<Integer>(),
            Ok((-9223372036854775808i64).into())
        );
        assert!("-9223372036854775809".parse::<Integer>().is_err());
        assert_eq!(
            "18446744073709551615".parse::<Integer>(),
            Ok(18446744073709551615u64.into())
        );
        assert!("18446744073709551616".parse::<Integer>().is_err());
    }
}
