//! # Plist
//!
//! A rusty plist parser.
//!
//! ## Usage
//!
//! Put this in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! plist = "0.5"
//! ```
//!
//! And put this in your crate root:
//!
//! ```rust
//! extern crate plist;
//! ```
//!
//! ## Examples
//!
//! ```rust
//! use plist::Value;
//!
//! let value = Value::from_file("tests/data/xml.plist").unwrap();
//!
//! match value {
//!     Value::Array(_array) => (),
//!     _ => ()
//! }
//! ```
//!
//! ```rust
//! extern crate plist;
//! # #[cfg(feature = "serde")]
//! #[macro_use]
//! extern crate serde_derive;
//!
//! # #[cfg(feature = "serde")]
//! # fn main() {
//! #[derive(Deserialize)]
//! #[serde(rename_all = "PascalCase")]
//! struct Info {
//!     author: String,
//!     height: f32,
//! }
//!
//! let info: Info = plist::from_file("tests/data/xml.plist").unwrap();
//! # }
//! #
//! # #[cfg(not(feature = "serde"))]
//! # fn main() {}
//! ```

pub mod dictionary;

#[cfg(feature = "enable_unstable_features_that_may_break_with_minor_version_bumps")]
pub mod stream;
#[cfg(not(feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"))]
mod stream;

mod date;
mod integer;
mod value;

pub use date::Date;
pub use dictionary::Dictionary;
pub use integer::Integer;
pub use value::Value;

// Optional serde module
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
#[cfg(feature = "serde")]
mod de;
#[cfg(feature = "serde")]
mod ser;
#[cfg(all(
    feature = "serde",
    any(
        test,
        feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
    )
))]
pub use self::{de::Deserializer, ser::Serializer};
#[cfg(feature = "serde")]
pub use self::{
    de::{from_file, from_reader, from_reader_xml},
    ser::{to_file_binary, to_file_xml, to_writer_binary, to_writer_xml},
};

#[cfg(all(test, feature = "serde"))]
#[macro_use]
extern crate serde_derive;

#[cfg(all(test, feature = "serde"))]
mod serde_tests;

use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    InvalidData,
    UnexpectedEof,
    Io(io::Error),
    Serde(String),
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::InvalidData => "invalid data",
            Error::UnexpectedEof => "unexpected eof",
            Error::Io(ref err) => err.description(),
            Error::Serde(ref err) => &err,
        }
    }

    fn cause(&self) -> Option<&dyn ::std::error::Error> {
        match *self {
            Error::Io(ref err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref err) => err.fmt(fmt),
            _ => <Self as ::std::error::Error>::description(self).fmt(fmt),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

fn u64_to_usize(len_u64: u64) -> Option<usize> {
    let len = len_u64 as usize;
    if len as u64 != len_u64 {
        return None; // Too long
    }
    Some(len)
}
