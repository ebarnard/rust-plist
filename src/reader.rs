use rustc_serialize::base64::FromBase64;
use std::io::Read;
use std::str::FromStr;
use xml::reader::ParserConfig;
use xml::reader::EventReader as XmlEventReader;
use xml::reader::events::XmlEvent;

#[derive(PartialEq, Debug, Clone)]
pub enum PlistEvent {
	StartArray,
	EndArray,

	StartDictionary,
	EndDictionary,
	DictionaryKey(String),

	BooleanValue(bool),
	DataValue(Vec<u8>),
	DateValue(String),
	FloatValue(f64),
	IntegerValue(i64),
	StringValue(String),

	Error(())
}

pub struct EventReader<R: Read> {
	xml_reader: XmlEventReader<R>
}

impl<R: Read> EventReader<R> {
	pub fn new(reader: R) -> EventReader<R> {
		let config = ParserConfig {
			trim_whitespace: false,
			whitespace_to_characters: true,
			cdata_to_characters: true,
			ignore_comments: true,
			coalesce_characters: true,
		};

		EventReader {
			xml_reader: XmlEventReader::with_config(reader, config)
		}
	}

	fn read_string<F>(&mut self, f: F) -> PlistEvent where F:FnOnce(String) -> PlistEvent {
		match self.xml_reader.next() {
			XmlEvent::Characters(s) => f(s),
			_ => PlistEvent::Error(())
		}
	}

	pub fn next(&mut self) -> Option<PlistEvent> {
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
							Ok(f) => PlistEvent::FloatValue(f),
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

#[cfg(test)]
mod tests {
	use std::fs::File;
	use std::path::Path;

	use reader::*;

	#[test]
	fn simple() {
		use reader::PlistEvent::*;

		let reader = File::open(&Path::new("./tests/data/simple.plist")).unwrap();
		let mut event_reader = EventReader::new(reader);
		let mut events = Vec::new();
		loop {
			if let Some(event) = event_reader.next() {
				events.push(event);
			} else {
				break;
			}
		}

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
			EndDictionary
		];

		assert_eq!(events, comparison);
	}
}