extern crate byteorder;
extern crate itertools;
extern crate rustc_serialize;
extern crate xml as xml_rs;

pub mod binary;
pub mod xml;

use byteorder::Error as ByteorderError;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::io::Error as IoError;
use std::string::FromUtf16Error;

#[derive(Clone, Debug, PartialEq)]
pub enum Plist {
	Array(Vec<Plist>),
	Dictionary(HashMap<String, Plist>),
	Boolean(bool),
	Data(Vec<u8>),
	Date(String),
	Real(f64),
	Integer(i64),
	String(String)
}

#[derive(Debug, PartialEq)]
pub enum PlistEvent {
	StartArray,
	EndArray,

	StartDictionary,
	EndDictionary,

	BooleanValue(bool),
	DataValue(Vec<u8>),
	DateValue(String),
	IntegerValue(i64),
	RealValue(f64),
	StringValue(String),

	Error(ParserError)
}

type ParserResult<T> = Result<T, ParserError>;

#[derive(Debug)]
pub enum ParserError {
	InvalidData,
	UnexpectedEof,
	UnsupportedType,
	Io(IoError)
}

// No two errors are the same - this is a bit annoying though
impl PartialEq for ParserError {
	fn eq(&self, other: &ParserError) -> bool {
		false
	}
}

impl From<IoError> for ParserError {
	fn from(io_error: IoError) -> ParserError {
		ParserError::Io(io_error)
	}
}

impl From<ByteorderError> for ParserError {
	fn from(err: ByteorderError) -> ParserError {
		match err {
			ByteorderError::UnexpectedEOF => ParserError::UnexpectedEof,
			ByteorderError::Io(err) => ParserError::Io(err)
		}
	}
}

impl From<FromUtf16Error> for ParserError {
	fn from(_: FromUtf16Error) -> ParserError {
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
	type Item = PlistEvent;

	fn next(&mut self) -> Option<PlistEvent> {
		match *self {
			StreamingParser::Xml(ref mut parser) => parser.next(),
			StreamingParser::Binary(ref mut parser) => parser.next()
		}
	}
}

pub type BuilderResult<T> = Result<T, BuilderError>;

#[derive(Debug, PartialEq)]
pub enum BuilderError {
	InvalidEvent,
	UnsupportedDictionaryKey,
	ParserError(ParserError)
}

impl From<ParserError> for BuilderError {
	fn from(err: ParserError) -> BuilderError {
		BuilderError::ParserError(err)
	}
}

pub struct Builder<T> {
	stream: T,
	token: Option<PlistEvent>,
}

impl<R: Read + Seek> Builder<StreamingParser<R>> {
	pub fn new(reader: R) -> Builder<StreamingParser<R>> {
		Builder::from_event_stream(StreamingParser::new(reader))
	}
}

impl<T:Iterator<Item=PlistEvent>> Builder<T> {
	pub fn from_event_stream(stream: T) -> Builder<T> {
		Builder {
			stream: stream,
			token: None
		}
	}

	pub fn build(mut self) -> BuilderResult<Plist> {
		self.bump();
		let plist = try!(self.build_value());
		self.bump();
		match self.token {
			None => (),
			// The stream should have finished
			_ => return Err(BuilderError::InvalidEvent)
		};
		Ok(plist)
	}

	fn bump(&mut self) {
		self.token = self.stream.next();
	}

	fn build_value(&mut self) -> BuilderResult<Plist> {
		match self.token.take() {
			Some(PlistEvent::StartArray) => Ok(Plist::Array(try!(self.build_array()))),
			Some(PlistEvent::StartDictionary) => Ok(Plist::Dictionary(try!(self.build_dict()))),

			Some(PlistEvent::BooleanValue(b)) => Ok(Plist::Boolean(b)),
			Some(PlistEvent::DataValue(d)) => Ok(Plist::Data(d)),
			Some(PlistEvent::DateValue(d)) => Ok(Plist::Date(d)),
			Some(PlistEvent::IntegerValue(i)) => Ok(Plist::Integer(i)),
			Some(PlistEvent::RealValue(f)) => Ok(Plist::Real(f)),
			Some(PlistEvent::StringValue(s)) => Ok(Plist::String(s)),

			Some(PlistEvent::EndArray) => Err(BuilderError::InvalidEvent),
			Some(PlistEvent::EndDictionary) => Err(BuilderError::InvalidEvent),
			Some(PlistEvent::Error(_)) => Err(BuilderError::InvalidEvent),
			// The stream should not have ended here
			None => Err(BuilderError::InvalidEvent)
		}
	}

	fn build_array(&mut self) -> Result<Vec<Plist>, BuilderError> {	
		let mut values = Vec::new();

		loop {
			self.bump();
			if let Some(PlistEvent::EndArray) = self.token {
				self.token.take();
				return Ok(values);
			}
			values.push(try!(self.build_value()));
		}
	}

	fn build_dict(&mut self) -> Result<HashMap<String, Plist>, BuilderError> {
		let mut values = HashMap::new();

		loop {
			
			self.bump();
			match self.token.take() {
				Some(PlistEvent::EndDictionary) => return Ok(values),
				Some(PlistEvent::StringValue(s)) => {
					self.bump();
					values.insert(s, try!(self.build_value()));
				},
				_ => {
					// Only string keys are supported in plists
					return Err(BuilderError::UnsupportedDictionaryKey)
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	#[test]
	fn builder() {
		use super::PlistEvent::*;

		// Input

		let events = vec![
			StartDictionary,
			StringValue("Author".to_owned()),
			StringValue("William Shakespeare".to_owned()),
			StringValue("Lines".to_owned()),
			StartArray,
			StringValue("It is a tale told by an idiot,".to_owned()),
			StringValue("Full of sound and fury, signifying nothing.".to_owned()),
			EndArray,
			StringValue("Birthdate".to_owned()),
			IntegerValue(1564),
			StringValue("Height".to_owned()),
			RealValue(1.60),
			EndDictionary
		];

		let builder = Builder::from_event_stream(events.into_iter());
		let plist = builder.build();

		// Expected output

		let mut lines = Vec::new();
		lines.push(Plist::String("It is a tale told by an idiot,".to_owned()));
		lines.push(Plist::String("Full of sound and fury, signifying nothing.".to_owned()));

		let mut dict = HashMap::new();
		dict.insert("Author".to_owned(), Plist::String("William Shakespeare".to_owned()));
		dict.insert("Lines".to_owned(), Plist::Array(lines));
		dict.insert("Birthdate".to_owned(), Plist::Integer(1564));
		dict.insert("Height".to_owned(), Plist::Real(1.60));

		assert_eq!(plist, Ok(Plist::Dictionary(dict)));
	}
}