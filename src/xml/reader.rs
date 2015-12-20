use chrono::{DateTime, UTC};
use rustc_serialize::base64::FromBase64;
use std::io::Read;
use std::str::FromStr;
use xml_rs::reader::{EventReader, ParserConfig, XmlEvent};

use super::super::{ParserError, ParserResult, PlistEvent};

pub struct StreamingParser<R: Read> {
	xml_reader: EventReader<R>,
	queued_event: Option<XmlEvent>,
	element_stack: Vec<String>,
	finished: bool
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
			xml_reader: EventReader::new_with_config(reader, config),
			queued_event: None,
			element_stack: Vec::new(),
			finished: false
		}
	}

	fn read_content<F>(&mut self, f: F) -> ParserResult<PlistEvent> where F:FnOnce(String) -> ParserResult<PlistEvent> {
		match self.xml_reader.next() {
			Ok(XmlEvent::Characters(s)) => f(s),
			Ok(event @ XmlEvent::EndElement{..}) => {
				self.queued_event = Some(event);
				f("".to_owned())
			},
			_ => Err(ParserError::InvalidData)
		}
	}

	fn next_event(&mut self) -> Result<XmlEvent, ()> {
		if let Some(event) = self.queued_event.take() {
			Ok(event)
		} else {
			self.xml_reader.next().map_err(|_| ())
		}
	}

	fn read_next(&mut self) -> Option<ParserResult<PlistEvent>> {
		loop {
			match self.next_event() {
				Ok(XmlEvent::StartElement { name, .. }) => {
					// Add the current element to the element stack
					self.element_stack.push(name.local_name.clone());
					
					match &name.local_name[..] {
						"plist" => return Some(Ok(PlistEvent::StartPlist)),
						"array" => return Some(Ok(PlistEvent::StartArray(None))),
						"dict" => return Some(Ok(PlistEvent::StartDictionary(None))),
						"key" => return Some(self.read_content(|s| Ok(PlistEvent::Key(s)))),
						"true" => return Some(Ok(PlistEvent::BooleanValue(true))),
						"false" => return Some(Ok(PlistEvent::BooleanValue(false))),
						"data" => return Some(self.read_content(|s| {
							let s: String = s.replace(" ", "").replace("\t", "");
							match FromBase64::from_base64(&s[..]) {
								Ok(b) => Ok(PlistEvent::DataValue(b)),
								Err(_) => Err(ParserError::InvalidData)
							}
						})),
						"date" => return Some(self.read_content(|s| {
							let date = try!(DateTime::parse_from_rfc3339(&s));
							Ok(PlistEvent::DateValue(date.with_timezone(&UTC)))
						})),
						"integer" => return Some(self.read_content(|s| {
							match FromStr::from_str(&s)	{
								Ok(i) => Ok(PlistEvent::IntegerValue(i)),
								Err(_) => Err(ParserError::InvalidData)
							}
						})),
						"real" => return Some(self.read_content(|s| {
							match FromStr::from_str(&s)	{
								Ok(f) => Ok(PlistEvent::RealValue(f)),
								Err(_) => Err(ParserError::InvalidData)
							}
						})),
						"string" => return Some(self.read_content(|s| Ok(PlistEvent::StringValue(s)))),
						_ => return Some(Err(ParserError::InvalidData))
					}
				},
				Ok(XmlEvent::EndElement { name, .. }) => {
					// Check the corrent element is being closed
					match self.element_stack.pop() {
						Some(ref open_name) if &name.local_name == open_name => (),
						Some(ref _open_name) => return Some(Err(ParserError::InvalidData)),
						None => return Some(Err(ParserError::InvalidData))
					}

					match &name.local_name[..] {
						"array" => return Some(Ok(PlistEvent::EndArray)),
						"dict" => return Some(Ok(PlistEvent::EndDictionary)),
						"plist" => return Some(Ok(PlistEvent::EndPlist)),
						_ => ()
					}
				},
				Ok(XmlEvent::EndDocument) => {
					match self.element_stack.is_empty() {
						true => return None,
						false => return Some(Err(ParserError::UnexpectedEof))
					}
				},
				Err(_) => return Some(Err(ParserError::InvalidData)),
				_ => ()
			}
		}
	}
}

impl<R: Read> Iterator for StreamingParser<R> {
	type Item = ParserResult<PlistEvent>;

	fn next(&mut self) -> Option<ParserResult<PlistEvent>> {
		if self.finished {
			None
		} else {
			match self.read_next() {
				Some(Ok(event)) => Some(Ok(event)),
				Some(Err(err)) => {
					self.finished = true;
					Some(Err(err))
				},
				None => {
					self.finished = true;
					None
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use chrono::{TimeZone, UTC};
	use std::fs::File;
	use std::path::Path;

	use super::*;
	use PlistEvent;

	#[test]
	fn streaming_parser() {
		use PlistEvent::*;

		let reader = File::open(&Path::new("./tests/data/xml.plist")).unwrap();
		let streaming_parser = StreamingParser::new(reader);
		let events: Vec<PlistEvent> = streaming_parser.map(|e| e.unwrap()).collect();

		let comparison = &[
			StartPlist,
			StartDictionary(None),
			Key("Author".to_owned()),
			StringValue("William Shakespeare".to_owned()),
			Key("Lines".to_owned()),
			StartArray(None),
			StringValue("It is a tale told by an idiot,".to_owned()),
			StringValue("Full of sound and fury, signifying nothing.".to_owned()),
			EndArray,
			Key("Death".to_owned()),
			IntegerValue(1564),
			Key("Height".to_owned()),
			RealValue(1.60),
			Key("Data".to_owned()),
			DataValue(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0]),
			Key("Birthdate".to_owned()),
			DateValue(UTC.ymd(1981, 05, 16).and_hms(11, 32, 06)),
			Key("Blank".to_owned()),
			StringValue("".to_owned()),
			EndDictionary,
			EndPlist
		];

		assert_eq!(events, comparison);
	}

	#[test]
	fn bad_data() {
		let reader = File::open(&Path::new("./tests/data/xml_error.plist")).unwrap();
		let streaming_parser = StreamingParser::new(reader);
		let events: Vec<_> = streaming_parser.collect();

		assert!(events.last().unwrap().is_err());
	}
}
