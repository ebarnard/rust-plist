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
//! plist = "0.2"
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
//! extern crate plist;
//!
//! use plist::Value;
//! use std::fs::File;
//!
//! let file = File::open("tests/data/xml.plist").unwrap();
//! let plist = Value::from_reader(file).unwrap();
//!
//! match plist {
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
//! use std::fs::File;
//!
//! #[derive(Deserialize)]
//! #[serde(rename_all = "PascalCase")]
//! struct Info {
//!     author: String,
//!     height: f32,
//! }
//!
//! let file = File::open("tests/data/xml.plist").unwrap();
//! let info: Info = plist::from_reader(file).unwrap();
//! # }
//! #
//! # #[cfg(not(feature = "serde"))]
//! # fn main() {}
//! ```

extern crate base64;
extern crate byteorder;
extern crate humantime;
extern crate xml as xml_rs;

pub mod events;

mod date;
mod value;

pub use date::Date;
pub use value::Value;

// Optional serde module
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
#[cfg(feature = "serde")]
mod de;
#[cfg(feature = "serde")]
mod ser;
#[cfg(feature = "serde")]
pub use self::de::{from_reader, Deserializer};
#[cfg(feature = "serde")]
pub use self::ser::{to_writer_xml, Serializer};

use std::fmt;
use std::io;

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

    fn cause(&self) -> Option<&::std::error::Error> {
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
