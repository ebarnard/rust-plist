use rustc_serialize::base64::FromBase64;
use std::collections::HashMap;
use std::io::Read;
use std::str::FromStr;
use xml::reader::{EventReader, ParserConfig};
use xml::reader::events::XmlEvent;

use super::Plist;

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

pub struct StreamingParser<R: Read> {
	xml_reader: EventReader<R>
}

impl<R: Read> StreamingParser<R> {
	pub fn new(reader: R) -> StreamingParser<R> {
		let config = ParserConfig {
			trim_whitespace: false,
			whitespace_to_characters: true,
			cdata_to_characters: true,
			ignore_comments: true,
			coalesce_characters: true,
		};

		StreamingParser {
			xml_reader: EventReader::with_config(reader, config)
		}
	}

	fn read_string<F>(&mut self, f: F) -> PlistEvent where F:FnOnce(String) -> PlistEvent {
		match self.xml_reader.next() {
			XmlEvent::Characters(s) => f(s),
			_ => PlistEvent::Error(())
		}
	}
}

impl<R: Read> Iterator for StreamingParser<R> {
	type Item = PlistEvent;

	fn next(&mut self) -> Option<PlistEvent> {
		loop {
			let first_event = self.xml_reader.next();
			match first_event {
				XmlEvent::StartElement { name, .. } => match &name.local_name[..] {
					"plist" => (),
					"array" => return Some(PlistEvent::StartArray),
					"dict" => return Some(PlistEvent::StartDictionary),
					"key" => return Some(self.read_string(|s| PlistEvent::DictionaryKey(s))),
					"true" => return Some(PlistEvent::BooleanValue(true)),
					"false" => return Some(PlistEvent::BooleanValue(false)),
					"data" => return Some(self.read_string(|s| {
						match FromBase64::from_base64(&s[..]) {
							Ok(b) => PlistEvent::DataValue(b),
							Err(_) => PlistEvent::Error(())
						}
					})),
					"date" => return Some(self.read_string(|s| PlistEvent::DateValue(s))),
					"integer" => return Some(self.read_string(|s| {
						match FromStr::from_str(&s)	{
							Ok(i) => PlistEvent::IntegerValue(i),
							Err(_) => PlistEvent::Error(())
						}
					})),
					"real" => return Some(self.read_string(|s| {
						match FromStr::from_str(&s)	{
							Ok(f) => PlistEvent::RealValue(f),
							Err(_) => PlistEvent::Error(())
						}
					})),
					"string" => return Some(self.read_string(|s| PlistEvent::StringValue(s))),
					_ => return Some(PlistEvent::Error(()))
				},
				XmlEvent::EndElement { name, .. } => match &name.local_name[..] {
					"array" => return Some(PlistEvent::EndArray),
					"dict" => return Some(PlistEvent::EndDictionary),
					_ => ()
				},
				XmlEvent::EndDocument => return None,
				_ => ()
			}
		}
	}
}

pub struct Parser<R: Read> {
	reader: StreamingParser<R>,
	token: Option<PlistEvent>,
}

impl<R: Read> Parser<R> {
	pub fn new(reader: R) -> Parser<R> {
		Parser {
			reader: StreamingParser::new(reader),
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
	use std::fs::File;
	use std::collections::HashMap;
	use std::path::Path;

	use reader::*;
	use super::super::Plist;

	#[test]
	fn streaming_parser() {
		use reader::PlistEvent::*;

		let reader = File::open(&Path::new("./tests/data/simple.plist")).unwrap();
		let streaming_parser = StreamingParser::new(reader);
		let events: Vec<PlistEvent> = streaming_parser.collect();

		let comparison = &[
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

		assert_eq!(events, comparison);
	}

	#[test]
	fn parser() {
		let reader = File::open(&Path::new("./tests/data/simple.plist")).unwrap();
		let parser = Parser::new(reader);
		let plist = parser.parse();

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