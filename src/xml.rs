use rustc_serialize::base64::FromBase64;
use std::io::{Read, Write};
use std::str::FromStr;
use xml_rs::namespace::Namespace;
use xml_rs::reader::{EventReader, ParserConfig};
use xml_rs::reader::events::XmlEvent;
use xml_rs::writer::{EventWriter, EmitterConfig};
use xml_rs::writer::events::XmlEvent as WriteXmlEvent;

use super::{ParserError, ParserResult, PlistEvent};

pub struct Writer<W: Write> {
	xml_writer: EventWriter<W>,
	// Not very nice
	empty_namespace: Namespace
}

impl<W: Write> Writer<W> {
	fn new(writer: W) -> Writer<W> {
		let config = EmitterConfig {
			line_separator: "\n".to_owned(),
			indent_string: "    ".to_owned(),
			perform_indent: true,
			write_document_declaration: true,
			normalize_empty_elements: true,
			cdata_to_characters: true,
		};

		Writer {
			xml_writer: EventWriter::new_with_config(writer, config),
			empty_namespace: Namespace::empty()
		}
	}

	fn write_element_and_value(&mut self, name: &str, value: &str) -> Result<(), ()> {
		try!(self.start_element(name));
		try!(self.write_value(value));
		try!(self.end_element(name));
		Ok(())
	}

	fn start_element(&mut self, name: &str) -> Result<(), ()> {
		let result = self.xml_writer.write(WriteXmlEvent::StartElement {
			name: ::xml_rs::name::Name::local(name),
			attributes: Vec::new(),
			namespace: &self.empty_namespace
		});

		match result {
			Ok(()) => Ok(()),
			Err(_) => Err(())
		}
	}

	fn end_element(&mut self, name: &str) -> Result<(), ()> {
		let result = self.xml_writer.write(WriteXmlEvent::EndElement {
			name: ::xml_rs::name::Name::local(name)
		});

		match result {
			Ok(()) => Ok(()),
			Err(_) => Err(())
		}
	}

	fn write_value(&mut self, value: &str) -> Result<(), ()> {
		let result = self.xml_writer.write(WriteXmlEvent::Characters(value));

		match result {
			Ok(()) => Ok(()),
			Err(_) => Err(())
		}
	}

	fn write(&mut self, event: PlistEvent) -> Result<(), ()> {
		Ok(match event {
			PlistEvent::StartPlist => try!(self.start_element("plist")),
			PlistEvent::EndPlist => try!(self.end_element("plist")),

			PlistEvent::StartArray(_) => try!(self.start_element("array")),
			PlistEvent::EndArray => try!(self.end_element("array")),

			PlistEvent::StartDictionary(_) => try!(self.start_element("dict")),
			PlistEvent::EndDictionary => try!(self.end_element("dict")),

			PlistEvent::BooleanValue(true) => {
				try!(self.start_element("true"));
				try!(self.end_element("true"));
			},
			PlistEvent::BooleanValue(false) => {
				try!(self.start_element("false"));
				try!(self.end_element("false"));
			},
			PlistEvent::DataValue(value) => {
				panic!();
			},
			PlistEvent::DateValue(value) => panic!("unimpl"),
			PlistEvent::IntegerValue(value) => try!(self.write_element_and_value("integer", &value.to_string())),
			PlistEvent::RealValue(value) => try!(self.write_element_and_value("real", &value.to_string())),
			PlistEvent::StringValue(value) => try!(self.write_element_and_value("string", &value)),
		})
	}
}


pub struct StreamingParser<R: Read> {
	xml_reader: EventReader<R>,
	element_stack: Vec<String>
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
			xml_reader: EventReader::with_config(reader, config),
			element_stack: Vec::new()
		}
	}

	fn read_content<F>(&mut self, f: F) -> ParserResult<PlistEvent> where F:FnOnce(String) -> ParserResult<PlistEvent> {
		match self.xml_reader.next() {
			XmlEvent::Characters(s) => f(s),
			_ => Err(ParserError::InvalidData)
		}
	}
}

impl<R: Read> Iterator for StreamingParser<R> {
	type Item = ParserResult<PlistEvent>;

	fn next(&mut self) -> Option<ParserResult<PlistEvent>> {
		loop {
			match self.xml_reader.next() {
				XmlEvent::StartElement { name, .. } => {
					// Add the current element to the element stack
					self.element_stack.push(name.local_name.clone());
					
					match &name.local_name[..] {
						"plist" => return Some(Ok(PlistEvent::StartPlist)),
						"array" => return Some(Ok(PlistEvent::StartArray(None))),
						"dict" => return Some(Ok(PlistEvent::StartDictionary(None))),
						"key" => return Some(self.read_content(|s| Ok(PlistEvent::StringValue(s)))),
						"true" => return Some(Ok(PlistEvent::BooleanValue(true))),
						"false" => return Some(Ok(PlistEvent::BooleanValue(false))),
						"data" => return Some(self.read_content(|s| {
							match FromBase64::from_base64(&s[..]) {
								Ok(b) => Ok(PlistEvent::DataValue(b)),
								Err(_) => Err(ParserError::InvalidData)
							}
						})),
						"date" => return Some(self.read_content(|s| Ok(PlistEvent::DateValue(s)))),
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
				XmlEvent::EndElement { name, .. } => {
					// Check the corrent element is being closed
					match self.element_stack.pop() {
						Some(ref open_name) if &name.local_name == open_name => (),
						Some(ref open_name) => return Some(Err(ParserError::InvalidData)),
						None => return Some(Err(ParserError::InvalidData))
					}

					match &name.local_name[..] {
						"array" => return Some(Ok(PlistEvent::EndArray)),
						"dict" => return Some(Ok(PlistEvent::EndDictionary)),
						"plist" => return Some(Ok(PlistEvent::EndPlist)),
						_ => ()
					}
				},
				XmlEvent::EndDocument => {
					match self.element_stack.is_empty() {
						true => return None,
						false => return Some(Err(ParserError::UnexpectedEof))
					}
				}
				_ => ()
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::fs::File;
	use std::path::Path;

	use super::*;
	use super::super::PlistEvent;

	#[test]
	fn streaming_parser() {
		use super::super::PlistEvent::*;

		let reader = File::open(&Path::new("./tests/data/xml.plist")).unwrap();
		let streaming_parser = StreamingParser::new(reader);
		let events: Vec<PlistEvent> = streaming_parser.map(|e| e.unwrap()).collect();

		let comparison = &[
			StartPlist,
			StartDictionary(None),
			StringValue("Author".to_owned()),
			StringValue("William Shakespeare".to_owned()),
			StringValue("Lines".to_owned()),
			StartArray(None),
			StringValue("It is a tale told by an idiot,".to_owned()),
			StringValue("Full of sound and fury, signifying nothing.".to_owned()),
			EndArray,
			StringValue("Birthdate".to_owned()),
			IntegerValue(1564),
			StringValue("Height".to_owned()),
			RealValue(1.60),
			StringValue("Data".to_owned()),
			DataValue(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0]),
			EndDictionary,
			EndPlist
		];

		assert_eq!(events, comparison);
	}
}