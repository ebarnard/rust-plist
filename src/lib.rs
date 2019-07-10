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
mod error;
mod integer;
mod uid;
mod value;

pub use date::Date;
pub use dictionary::Dictionary;
pub use error::Error;
pub use integer::Integer;
pub use uid::Uid;
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

fn u64_to_usize(len_u64: u64) -> Option<usize> {
    let len = len_u64 as usize;
    if len as u64 != len_u64 {
        return None; // Too long
    }
    Some(len)
}
