use rustc_serialize::base64::{MIME, ToBase64};
use std::io::Write;
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

	pub fn write(&mut self, event: PlistEvent) -> Result<(), ()> {
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
				let base64_data = value.to_base64(MIME);
				try!(self.write_element_and_value("data", &base64_data));
			}
			PlistEvent::DateValue(_value) => panic!("unimpl"),
			PlistEvent::IntegerValue(value) => try!(self.write_element_and_value("integer", &value.to_string())),
			PlistEvent::RealValue(value) => try!(self.write_element_and_value("real", &value.to_string())),
			PlistEvent::StringValue(value) => try!(self.write_element_and_value("string", &value)),
		})
	}
}