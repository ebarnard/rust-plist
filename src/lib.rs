extern crate byteorder;
extern crate chrono;
extern crate rustc_serialize;
extern crate xml as xml_rs;

pub mod binary;
pub mod xml;
mod builder;

use chrono::{DateTime, UTC};
use chrono::format::ParseError as ChronoParseError;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::io::Error as IoError;

#[derive(Clone, Debug, PartialEq)]
pub enum Plist {
	Array(Vec<Plist>),
	Dictionary(BTreeMap<String, Plist>),
	Boolean(bool),
	Data(Vec<u8>),
	Date(DateTime<UTC>),
	Real(f64),
	Integer(i64),
	String(String)
}
		
use rustc_serialize::base64::{STANDARD, ToBase64};
use rustc_serialize::json::Json as RustcJson;

impl Plist {
	pub fn from_events<T>(events: T) -> Result<Plist, ()>
		where T: IntoIterator<Item = ReadResult<PlistEvent>>
	{
		let iter = events.into_iter();
		let builder = builder::Builder::new(iter);

		match builder.build() {
			Ok(plist) => Ok(plist),
			Err(_) => Err(())
		}
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
			},
			Plist::Dictionary(dict) => {
				events.push(PlistEvent::StartDictionary(Some(dict.len() as u64)));
				for (key, value) in dict.into_iter() {
					events.push(PlistEvent::StringValue(key));
					value.into_events_inner(events);
				}
				events.push(PlistEvent::EndDictionary);
			},
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
			Plist::Array(value) => RustcJson::Array(value.into_iter().map(|p| p.into_rustc_serialize_json()).collect()),
			Plist::Dictionary(value) => RustcJson::Object(value.into_iter().map(|(k, v)| (k, v.into_rustc_serialize_json())).collect()),
			Plist::Boolean(value) => RustcJson::Boolean(value),
			Plist::Data(value) => RustcJson::String(value.to_base64(STANDARD)),
			Plist::Date(value) => RustcJson::String(value.to_rfc3339()),
			Plist::Real(value) => RustcJson::F64(value),
			Plist::Integer(value) => RustcJson::I64(value),
			Plist::String(value) => RustcJson::String(value),
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlistEvent {
	StartPlist,
	EndPlist,

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

pub type ReadResult<T> = Result<T, ReadError>;

#[derive(Debug)]
pub enum ReadError {
	InvalidData,
	UnexpectedEof,
	UnsupportedType,
	Io(IoError)
}

impl From<IoError> for ReadError {
	fn from(io_error: IoError) -> ReadError {
		ReadError::Io(io_error)
	}
}

impl From<ChronoParseError> for ReadError {
	fn from(_: ChronoParseError) -> ReadError {
		ReadError::InvalidData
	}
}

pub enum EventReader<R: Read+Seek> {
	Uninitialized(Option<R>),
	Xml(xml::EventReader<R>),
	Binary(binary::EventReader<R>)
}

impl<R: Read+Seek> EventReader<R> {
	pub fn new(reader: R) -> EventReader<R> {
		EventReader::Uninitialized(Some(reader))
	}

	fn is_binary(reader: &mut R) -> Result<bool, IoError> {
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

impl<R: Read+Seek> Iterator for EventReader<R> {
	type Item = ReadResult<PlistEvent>;

	fn next(&mut self) -> Option<ReadResult<PlistEvent>> {
		let mut reader = match *self {
			EventReader::Xml(ref mut parser) => return parser.next(),
			EventReader::Binary(ref mut parser) => return parser.next(),
			EventReader::Uninitialized(ref mut reader) => reader.take().unwrap()
		};

		let event_reader = match EventReader::is_binary(&mut reader) {
			Ok(true) => EventReader::Binary(binary::EventReader::new(reader)),
			Ok(false) => EventReader::Xml(xml::EventReader::new(reader)),
			Err(err) => {
				::std::mem::replace(self, EventReader::Uninitialized(Some(reader)));
				return Some(Err(ReadError::Io(err)))
			}
		};

		::std::mem::replace(self, event_reader);

		self.next()
	}
}