use rustc_serialize::base64::{MIME, ToBase64};
use std::io::Write;
use xml_rs::attribute::Attribute;
use xml_rs::name::Name;
use xml_rs::namespace::Namespace;
use xml_rs::writer::{EventWriter, EmitterConfig};
use xml_rs::writer::events::XmlEvent as WriteXmlEvent;

use {PlistEvent};

pub struct Writer<W: Write> {
	xml_writer: EventWriter<W>,
	// Not very nice
	empty_namespace: Namespace
}

impl<W: Write> Writer<W> {
	pub fn new(writer: W) -> Writer<W> {
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
			name: Name::local(name),
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
			name: Name::local(name)
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
		Ok(match *event {
			PlistEvent::StartPlist => {
				let version_name = Name::local("version");
				let version_attr = Attribute::new(version_name, "1.0");

				let result = self.xml_writer.write(WriteXmlEvent::StartElement {
					name: Name::local("plist"),
					attributes: vec!(version_attr),
					namespace: &self.empty_namespace
				});

				match result {
					Ok(()) => (),
					Err(_) => return Err(())
				}
			},
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
	use std::fs::File;
	use std::path::Path;

	use super::*;
	use PlistEvent;

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
			let mut plist_w = Writer::new(&mut cursor);

			for item in plist {
				plist_w.write(item).unwrap();
			}
		}

		let comparison = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<plist version=\"1.0\"><dict><string>Author</string>
<string>William Shakespeare</string>
<string>Lines</string>
<array><string>It is a tale told by an idiot,</string>
<string>Full of sound and fury, signifying nothing.</string></array>
<string>Death</string>
<integer>1564</integer>
<string>Height</string>
<real>1.6</real>
<string>Data</string>
<data>AAAAvgAAAAMAAAAeAAAA</data>
<string>Birthdate</string>
<date>1981-05-16T11:32:06+00:00</date></dict></plist>";


		let s = String::from_utf8(cursor.into_inner()).unwrap();

		assert_eq!(&s, comparison);
	}
}