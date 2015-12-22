use rustc_serialize::base64::{MIME, ToBase64};
use std::borrow::Cow;
use std::io::Write;
use xml_rs::attribute::Attribute;
use xml_rs::name::Name;
use xml_rs::namespace::Namespace;
use xml_rs::writer::{EventWriter as XmlEventWriter, EmitterConfig};
use xml_rs::writer::events::XmlEvent as WriteXmlEvent;

use {PlistEvent};

enum Element {
	Dictionary(DictionaryState),
	Array,
	Root
}

enum DictionaryState {
	ExpectKey,
	ExpectValue
}

pub struct EventWriter<W: Write> {
	xml_writer: XmlEventWriter<W>,
	stack: Vec<Element>,
	// Not very nice
	empty_namespace: Namespace
}

impl<W: Write> EventWriter<W> {
	pub fn new(writer: W) -> EventWriter<W> {
		let config = EmitterConfig {
			line_separator: "\n".into(),
			indent_string: "    ".into(),
			perform_indent: true,
			perform_escaping: true,
			write_document_declaration: true,
			normalize_empty_elements: true,
			cdata_to_characters: true,
			keep_element_names_stack: false,
			autopad_comments: true
		};

		EventWriter {
			xml_writer: XmlEventWriter::new_with_config(writer, config),
			stack: Vec::new(),
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
			name: Name::local(name),
			attributes: Cow::Borrowed(&[]),
			namespace: Cow::Borrowed(&self.empty_namespace)
		});

		match result {
			Ok(()) => Ok(()),
			Err(_) => Err(())
		}
	}

	fn end_element(&mut self, name: &str) -> Result<(), ()> {
		let result = self.xml_writer.write(WriteXmlEvent::EndElement {
			name: Some(Name::local(name))
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

	pub fn write(&mut self, event: &PlistEvent) -> Result<(), ()> {
		match self.stack.pop() {
			Some(Element::Dictionary(DictionaryState::ExpectKey)) => {
				match *event {
					PlistEvent::StringValue(ref value) => {
						try!(self.write_element_and_value("key", &*value));
						self.stack.push(Element::Dictionary(DictionaryState::ExpectValue));
					}
					PlistEvent::EndDictionary => try!(self.end_element("dict")),
					_ => return Err(()) // Invalid event
				};
				return Ok(())
			},
			Some(Element::Dictionary(DictionaryState::ExpectValue)) => self.stack.push(Element::Dictionary(DictionaryState::ExpectKey)),
			Some(other) => self.stack.push(other),
			None => match *event {
				PlistEvent::StartPlist => {
					let version_name = Name::local("version");
					let version_attr = Attribute::new(version_name, "1.0");

					let result = self.xml_writer.write(WriteXmlEvent::StartElement {
						name: Name::local("plist"),
						attributes: Cow::Borrowed(&[version_attr]),
						namespace: Cow::Borrowed(&self.empty_namespace)
					});

					match result {
						Ok(()) => (),
						Err(_) => return Err(())
					}

					self.stack.push(Element::Root);
					return Ok(())
				},
				_ => return Err(()) // Invalid event
			}
		}

		Ok(match *event {
			PlistEvent::StartPlist => return Err(()), // Invalid event
			PlistEvent::EndPlist => {
				try!(self.end_element("plist"));
				if let Some(Element::Root) = self.stack.pop() {} else {
					return Err(()); // Invalid event
				}
			},

			PlistEvent::StartArray(_) => {
				try!(self.start_element("array"));
				self.stack.push(Element::Array);
			}
			PlistEvent::EndArray => {
				try!(self.end_element("array"));
				if let Some(Element::Array) = self.stack.pop() {} else {
					return Err(()); // Invalid event
				}
			}

			PlistEvent::StartDictionary(_) => {
				try!(self.start_element("dict"));
				self.stack.push(Element::Dictionary(DictionaryState::ExpectKey));
			}
			PlistEvent::EndDictionary => return Err(()), // Invalid event

			PlistEvent::BooleanValue(true) => {
				try!(self.start_element("true"));
				try!(self.end_element("true"));
			},
			PlistEvent::BooleanValue(false) => {
				try!(self.start_element("false"));
				try!(self.end_element("false"));
			},
			PlistEvent::DataValue(ref value) => {
				let base64_data = value.to_base64(MIME);
				try!(self.write_element_and_value("data", &base64_data));
			}
			PlistEvent::DateValue(ref value) => {
				let date = value.to_rfc3339();
				try!(self.write_element_and_value("date", &date));
			},
			PlistEvent::IntegerValue(ref value) => try!(self.write_element_and_value("integer", &value.to_string())),
			PlistEvent::RealValue(ref value) => try!(self.write_element_and_value("real", &value.to_string())),
			PlistEvent::StringValue(ref value) => try!(self.write_element_and_value("string", &*value)),
		})
	}
}

#[cfg(test)]
mod tests {
	use chrono::{TimeZone, UTC};
	use std::io::Cursor;

	use super::*;

	#[test]
	fn streaming_parser() {
		use PlistEvent::*;

		let plist = &[
			StartPlist,
			StartDictionary(None),
			StringValue("Author".to_owned()),
			StringValue("William Shakespeare".to_owned()),
			StringValue("Lines".to_owned()),
			StartArray(None),
			StringValue("It is a tale told by an idiot,".to_owned()),
			StringValue("Full of sound and fury, signifying nothing.".to_owned()),
			EndArray,
			StringValue("Death".to_owned()),
			IntegerValue(1564),
			StringValue("Height".to_owned()),
			RealValue(1.60),
			StringValue("Data".to_owned()),
			DataValue(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0]),
			StringValue("Birthdate".to_owned()),
			DateValue(UTC.ymd(1981, 05, 16).and_hms(11, 32, 06)),
			EndDictionary,
			EndPlist
		];

		let mut cursor = Cursor::new(Vec::new());

		{
			let mut plist_w = EventWriter::new(&mut cursor);

			for item in plist {
				plist_w.write(item).unwrap();
			}
		}

		let comparison = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<plist version=\"1.0\">
    <dict>
        <key>Author</key>
        <string>William Shakespeare</string>
        <key>Lines</key>
        <array>
            <string>It is a tale told by an idiot,</string>
            <string>Full of sound and fury, signifying nothing.</string>
        </array>
        <key>Death</key>
        <integer>1564</integer>
        <key>Height</key>
        <real>1.6</real>
        <key>Data</key>
        <data>AAAAvgAAAAMAAAAeAAAA</data>
        <key>Birthdate</key>
        <date>1981-05-16T11:32:06+00:00</date>
    </dict>
</plist>";


		let s = String::from_utf8(cursor.into_inner()).unwrap();

		assert_eq!(&s, comparison);
	}
}