extern crate byteorder;
extern crate chrono;
extern crate itertools;
extern crate rustc_serialize;
extern crate xml as xml_rs;

pub mod binary;
pub mod xml;
mod builder;

pub use builder::{Builder, BuilderError, BuilderResult};

use chrono::{DateTime, UTC};
use chrono::format::ParseError as ChronoParseError;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::io::Error as IoError;

#[derive(Clone, Debug, PartialEq)]
pub enum Plist {
	Array(Vec<Plist>),
	Dictionary(HashMap<String, Plist>),
	Boolean(bool),
	Data(Vec<u8>),
	Date(DateTime<UTC>),
	Real(f64),
	Integer(i64),
	String(String)
}

#[derive(Debug, PartialEq)]
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

pub type ParserResult<T> = Result<T, ParserError>;

#[derive(Debug)]
pub enum ParserError {
	InvalidData,
	UnexpectedEof,
	UnsupportedType,
	Io(IoError)
}

impl From<IoError> for ParserError {
	fn from(io_error: IoError) -> ParserError {
		ParserError::Io(io_error)
	}
}

impl From<ChronoParseError> for ParserError {
	fn from(_: ChronoParseError) -> ParserError {
		ParserError::InvalidData
	}
}

pub enum StreamingParser<R: Read+Seek> {
	Xml(xml::StreamingParser<R>),
	Binary(binary::StreamingParser<R>)
}

impl<R: Read+Seek> StreamingParser<R> {
	pub fn new(mut reader: R) -> StreamingParser<R> {
		match StreamingParser::is_binary(&mut reader) {
			Ok(true) => StreamingParser::Binary(binary::StreamingParser::new(reader)),
			Ok(false) | Err(_) => StreamingParser::Xml(xml::StreamingParser::new(reader))
		}
	}

	fn is_binary(reader: &mut R) -> Result<bool, IoError> {
		try!(reader.seek(SeekFrom::Start(0)));
		let mut magic = [0; 8];
		try!(reader.read(&mut magic));

		Ok(if &magic == b"bplist00" {
			true
		} else {
			false
		})
	}
}

impl<R: Read+Seek> Iterator for StreamingParser<R> {
	type Item = ParserResult<PlistEvent>;

	fn next(&mut self) -> Option<ParserResult<PlistEvent>> {
		match *self {
			StreamingParser::Xml(ref mut parser) => parser.next(),
			StreamingParser::Binary(ref mut parser) => parser.next()
		}
	}
}