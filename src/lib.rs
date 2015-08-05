extern crate rustc_serialize;
extern crate xml as xml_rs;

pub mod xml;

use std::collections::HashMap;
use std::io::Read;

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

#[derive(Clone, Debug, PartialEq)]
pub enum PlistEvent {
	StartArray,
	EndArray,

	StartDictionary,
	EndDictionary,
	DictionaryKey(String),

	BooleanValue(bool),
	DataValue(Vec<u8>),
	DateValue(String),
	IntegerValue(i64),
	RealValue(f64),
	StringValue(String),

	Error(())
}


pub enum StreamingParser<R: Read> {
	Xml(xml::StreamingParser<R>)
}

impl<R:Read> StreamingParser<R> {
	pub fn new() -> StreamingParser<R> {
		panic!()
	}
}

impl<R:Read> Iterator for StreamingParser<R> {
	type Item = PlistEvent;

	fn next(&mut self) -> Option<PlistEvent> {
		match *self {
			StreamingParser::Xml(ref mut parser) => parser.next()
		}
	}
}

pub struct Parser<T> {
	reader: T,
	token: Option<PlistEvent>,
}

impl<R:Read> Parser<StreamingParser<R>> {
	pub fn new(reader: R) -> Parser<StreamingParser<R>> {
		Parser::from_event_stream(StreamingParser::Xml(xml::StreamingParser::new(reader)))
	}
}

impl<T:Iterator<Item=PlistEvent>> Parser<T> {
	pub fn from_event_stream(stream: T) -> Parser<T> {
		Parser {
			reader: stream,
			token: None
		}
	}

	pub fn parse(mut self) -> Result<Plist, ()> {
		self.bump();
		let plist = try!(self.build_value());
		self.bump();
		match self.token {
			None => (),
			_ => return Err(())
		};
		Ok(plist)
	}

	fn bump(&mut self) {
		self.token = self.reader.next();
	}

	fn build_value(&mut self) -> Result<Plist, ()> {
		match self.token.take() {
			Some(PlistEvent::StartArray) => Ok(Plist::Array(try!(self.build_array()))),
			Some(PlistEvent::StartDictionary) => Ok(Plist::Dictionary(try!(self.build_dict()))),

			Some(PlistEvent::BooleanValue(b)) => Ok(Plist::Boolean(b)),
			Some(PlistEvent::DataValue(d)) => Ok(Plist::Data(d)),
			Some(PlistEvent::DateValue(d)) => Ok(Plist::Date(d)),
			Some(PlistEvent::IntegerValue(i)) => Ok(Plist::Integer(i)),
			Some(PlistEvent::RealValue(f)) => Ok(Plist::Real(f)),
			Some(PlistEvent::StringValue(s)) => Ok(Plist::String(s)),

			Some(PlistEvent::EndArray) => Err(()),
			Some(PlistEvent::EndDictionary) => Err(()),
			Some(PlistEvent::DictionaryKey(_)) => Err(()),
			Some(PlistEvent::Error(_)) => Err(()),
			None => Err(())
		}
	}

	fn build_array(&mut self) -> Result<Vec<Plist>, ()> {	
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

	fn build_dict(&mut self) -> Result<HashMap<String, Plist>, ()> {
		let mut values = HashMap::new();

		loop {
			
			self.bump();
			match self.token.take() {
				Some(PlistEvent::EndDictionary) => return Ok(values),
				Some(PlistEvent::DictionaryKey(s)) => {
					self.bump();
					values.insert(s, try!(self.build_value()));
				},
				_ => {
					return Err(())
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
	fn parser() {
		use super::PlistEvent::*;

		// Input

		let events = &[
			StartDictionary,
			DictionaryKey("Author".to_owned()),
			StringValue("William Shakespeare".to_owned()),
			DictionaryKey("Lines".to_owned()),
			StartArray,
			StringValue("It is a tale told by an idiot,".to_owned()),
			StringValue("Full of sound and fury, signifying nothing.".to_owned()),
			EndArray,
			DictionaryKey("Birthdate".to_owned()),
			IntegerValue(1564),
			DictionaryKey("Height".to_owned()),
			RealValue(1.60),
			EndDictionary
		];

		let parser = Parser::from_event_stream(events.into_iter().cloned());
		let plist = parser.parse();

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