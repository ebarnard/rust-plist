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
//! plist = "0.0.12"
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
//! ```rust ignore
//! use plist::Plist;
//! use std::fs::File;
//!
//! let file = File::open("tests/data/xml.plist").unwrap();
//! let plist = Plist::read(file).unwrap();
//!
//! match plist {
//!     Plist::Array(_array) => (),
//!     _ => ()
//! }
//!
//! ```
//!
//!

extern crate byteorder;
extern crate chrono;
extern crate rustc_serialize;
extern crate serde;
extern crate xml as xml_rs;

pub mod binary;
pub mod xml;

mod builder;
mod de;
mod ser;

pub use de::{Deserializer, DeserializeError};
pub use ser::Serializer;

use chrono::{DateTime, UTC};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::io::Error as IoError;

pub fn deserialize<R: Read + Seek, T: Deserialize>
    (reader: R)
     -> ::std::result::Result<T, DeserializeError> {
    let reader = EventReader::new(reader);
    let mut de = Deserializer::new(reader);
    Deserialize::deserialize(&mut de)
}

pub fn serialize_to_xml<W: Write, T: Serialize>(writer: W, value: &T) -> Result<()> {
    let writer = xml::EventWriter::new(writer);
    let mut ser = Serializer::new(writer);
    value.serialize(&mut ser)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Plist {
    Array(Vec<Plist>),
    Dictionary(BTreeMap<String, Plist>),
    Boolean(bool),
    Data(Vec<u8>),
    Date(DateTime<UTC>),
    Real(f64),
    Integer(i64),
    String(String),
}

use rustc_serialize::base64::{STANDARD, ToBase64};
use rustc_serialize::json::Json as RustcJson;

impl Plist {
    pub fn read<R: Read + Seek>(reader: R) -> Result<Plist> {
        let reader = EventReader::new(reader);
        Plist::from_events(reader)
    }

    pub fn from_events<T>(events: T) -> Result<Plist>
        where T: IntoIterator<Item = Result<PlistEvent>>
    {
        let iter = events.into_iter();
        let builder = builder::Builder::new(iter);
        builder.build()
    }

    pub fn into_events(self) -> Vec<PlistEvent> {
        let mut events = Vec::new();
        self.into_events_inner(&mut events);
        events
    }

    fn into_events_inner(self, events: &mut Vec<PlistEvent>) {
        match self {
            Plist::Array(array) => {
                events.push(PlistEvent::StartArray(Some(array.len() as u64)));
                for value in array.into_iter() {
                    value.into_events_inner(events);
                }
                events.push(PlistEvent::EndArray);
            }
            Plist::Dictionary(dict) => {
                events.push(PlistEvent::StartDictionary(Some(dict.len() as u64)));
                for (key, value) in dict.into_iter() {
                    events.push(PlistEvent::StringValue(key));
                    value.into_events_inner(events);
                }
                events.push(PlistEvent::EndDictionary);
            }
            Plist::Boolean(value) => events.push(PlistEvent::BooleanValue(value)),
            Plist::Data(value) => events.push(PlistEvent::DataValue(value)),
            Plist::Date(value) => events.push(PlistEvent::DateValue(value)),
            Plist::Real(value) => events.push(PlistEvent::RealValue(value)),
            Plist::Integer(value) => events.push(PlistEvent::IntegerValue(value)),
            Plist::String(value) => events.push(PlistEvent::StringValue(value)),
        }
    }

    pub fn into_rustc_serialize_json(self) -> RustcJson {
        match self {
            Plist::Array(value) => {
                RustcJson::Array(value.into_iter().map(|p| p.into_rustc_serialize_json()).collect())
            }
            Plist::Dictionary(value) => {
                RustcJson::Object(value.into_iter()
                                       .map(|(k, v)| (k, v.into_rustc_serialize_json()))
                                       .collect())
            }
            Plist::Boolean(value) => RustcJson::Boolean(value),
            Plist::Data(value) => RustcJson::String(value.to_base64(STANDARD)),
            Plist::Date(value) => RustcJson::String(value.to_rfc3339()),
            Plist::Real(value) => RustcJson::F64(value),
            Plist::Integer(value) => RustcJson::I64(value),
            Plist::String(value) => RustcJson::String(value),
        }
    }
}

impl Plist {
    /// If the `Plist` is an Array, returns the associated Vec.
    /// Returns None otherwise.
    pub fn as_array(&self) -> Option<&Vec<Plist>> {
        match self {
            &Plist::Array(ref array) => Some(array),
            _ => None,
        }
    }

    /// If the `Plist` is an Array, returns the associated mutable Vec.
    /// Returns None otherwise.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Plist>> {
        match self {
            &mut Plist::Array(ref mut array) => Some(array),
            _ => None,
        }
    }

    /// If the `Plist` is a Dictionary, returns the associated BTreeMap.
    /// Returns None otherwise.
    pub fn as_dictionary(&self) -> Option<&BTreeMap<String, Plist>> {
        match self {
            &Plist::Dictionary(ref map) => Some(map),
            _ => None,
        }
    }

    /// If the `Plist` is a Dictionary, returns the associated mutable BTreeMap.
    /// Returns None otherwise.
    pub fn as_dictionary_mut(&mut self) -> Option<&mut BTreeMap<String, Plist>> {
        match self {
            &mut Plist::Dictionary(ref mut map) => Some(map),
            _ => None,
        }
    }

    /// If the `Plist` is a Boolean, returns the associated bool.
    /// Returns None otherwise.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            &Plist::Boolean(v) => Some(v),
            _ => None,
        }
    }

    /// If the `Plist` is a Data, returns the associated Vec.
    /// Returns None otherwise.
    pub fn as_data(&self) -> Option<&Vec<u8>> {
        match self {
            &Plist::Data(ref data) => Some(data),
            _ => None,
        }
    }

    /// If the `Plist` is a Data, returns the associated mutable Vec.
    /// Returns None otherwise.
    pub fn as_data_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            &mut Plist::Data(ref mut data) => Some(data),
            _ => None,
        }
    }

    /// If the `Plist` is a Date, returns the associated DateTime.
    /// Returns None otherwise.
    pub fn as_date(&self) -> Option<DateTime<UTC>> {
        match self {
            &Plist::Date(date) => Some(date),
            _ => None,
        }
    }

    /// If the `Plist` is a Real, returns the associated f64.
    /// Returns None otherwise.
    pub fn as_real(&self) -> Option<f64> {
        match self {
            &Plist::Real(v) => Some(v),
            _ => None,
        }
    }

    /// If the `Plist` is an Integer, returns the associated i64.
    /// Returns None otherwise.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            &Plist::Integer(v) => Some(v),
            _ => None,
        }
    }

    /// If the `Plist` is a String, returns the associated str.
    /// Returns None otherwise.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            &Plist::String(ref v) => Some(v),
            _ => None,
        }
    }
}

/// An encoding of a plist as a flat structure.
///
/// Output by the event readers.
///
/// Dictionary keys and values are represented as pairs of values e.g.:
///
/// ```ignore rust
/// StartDictionary
/// StringValue("Height") // Key
/// RealValue(181.2)      // Value
/// StringValue("Age")    // Key
/// IntegerValue(28)      // Value
/// EndDictionary
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum PlistEvent {
    // While the length of an array or dict cannot be feasably greater than max(usize) this better
    // conveys the concept of an effectively unbounded event stream.
    StartArray(Option<u64>),
    EndArray,

    StartDictionary(Option<u64>),
    EndDictionary,

    BooleanValue(bool),
    DataValue(Vec<u8>),
    DateValue(DateTime<UTC>),
    IntegerValue(i64),
    RealValue(f64),
    StringValue(String),
}

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InvalidData,
    UnexpectedEof,
    Io(IoError),
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::InvalidData => "invalid data",
            Error::UnexpectedEof => "unexpected eof",
            Error::Io(ref err) => err.description(),
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

impl From<IoError> for Error {
    fn from(err: IoError) -> Error {
        Error::Io(err)
    }
}

pub struct EventReader<R: Read + Seek>(EventReaderInner<R>);

enum EventReaderInner<R: Read + Seek> {
    Uninitialized(Option<R>),
    Xml(xml::EventReader<R>),
    Binary(binary::EventReader<R>),
}

impl<R: Read + Seek> EventReader<R> {
    pub fn new(reader: R) -> EventReader<R> {
        EventReader(EventReaderInner::Uninitialized(Some(reader)))
    }

    fn is_binary(reader: &mut R) -> Result<bool> {
        try!(reader.seek(SeekFrom::Start(0)));
        let mut magic = [0; 8];
        try!(reader.read(&mut magic));
        try!(reader.seek(SeekFrom::Start(0)));

        Ok(if &magic == b"bplist00" {
            true
        } else {
            false
        })
    }
}

impl<R: Read + Seek> Iterator for EventReader<R> {
    type Item = Result<PlistEvent>;

    fn next(&mut self) -> Option<Result<PlistEvent>> {
        let mut reader = match self.0 {
            EventReaderInner::Xml(ref mut parser) => return parser.next(),
            EventReaderInner::Binary(ref mut parser) => return parser.next(),
            EventReaderInner::Uninitialized(ref mut reader) => reader.take().unwrap(),
        };

        let event_reader = match EventReader::is_binary(&mut reader) {
            Ok(true) => EventReaderInner::Binary(binary::EventReader::new(reader)),
            Ok(false) => EventReaderInner::Xml(xml::EventReader::new(reader)),
            Err(err) => {
                ::std::mem::replace(&mut self.0, EventReaderInner::Uninitialized(Some(reader)));
                return Some(Err(err));
            }
        };

        ::std::mem::replace(&mut self.0, event_reader);

        self.next()
    }
}

pub trait EventWriter {
    fn write(&mut self, event: &PlistEvent) -> Result<()>;
}

fn u64_to_usize(len_u64: u64) -> Result<usize> {
    let len = len_u64 as usize;
    if len as u64 != len_u64 {
        return Err(Error::InvalidData); // Too long
    }
    Ok(len)
}

fn u64_option_to_usize(len: Option<u64>) -> Result<Option<usize>> {
    match len {
        Some(len) => Ok(Some(try!(u64_to_usize(len)))),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::Plist;

    #[test]
    fn test_plist_access() {
        use std::collections::BTreeMap;
        use chrono::*;

        let vec = vec![Plist::Real(0.0)];
        let mut array = Plist::Array(vec.clone());
        assert_eq!(array.as_array(), Some(&vec.clone()));
        assert_eq!(array.as_array_mut(), Some(&mut vec.clone()));

        let mut map = BTreeMap::new();
        map.insert("key1".to_owned(), Plist::String("value1".to_owned()));
        let mut dict = Plist::Dictionary(map.clone());
        assert_eq!(dict.as_dictionary(), Some(&map.clone()));
        assert_eq!(dict.as_dictionary_mut(), Some(&mut map.clone()));

        assert_eq!(Plist::Boolean(true).as_boolean(), Some(true));

        let vec = vec![1, 2, 3];
        let mut data = Plist::Data(vec.clone());
        assert_eq!(data.as_data(), Some(&vec.clone()));
        assert_eq!(data.as_data_mut(), Some(&mut vec.clone()));

        let date: DateTime<UTC> = UTC::now();
        assert_eq!(Plist::Date(date).as_date(), Some(date));

        assert_eq!(Plist::Real(0.0).as_real(), Some(0.0));
        assert_eq!(Plist::Integer(1).as_integer(), Some(1));
        assert_eq!(Plist::String("2".to_owned()).as_string(), Some("2"));
    }
}
