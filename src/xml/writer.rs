use base64;
use std::borrow::Cow;
use std::io::Write;
use xml_rs::name::Name;
use xml_rs::namespace::Namespace;
use xml_rs::writer::events::XmlEvent as WriteXmlEvent;
use xml_rs::writer::{EmitterConfig, Error as XmlWriterError, EventWriter as XmlEventWriter};

use {Error, EventWriter as PlistEventWriter, PlistEvent, Result};

impl From<XmlWriterError> for Error {
    fn from(err: XmlWriterError) -> Error {
        match err {
            XmlWriterError::Io(err) => Error::Io(err),
            _ => Error::InvalidData,
        }
    }
}

enum Element {
    Dictionary(DictionaryState),
    Array,
    Root,
}

enum DictionaryState {
    ExpectKey,
    ExpectValue,
}

pub struct EventWriter<W: Write> {
    xml_writer: XmlEventWriter<W>,
    stack: Vec<Element>,
    // Not very nice
    empty_namespace: Namespace,
}

impl<W: Write> EventWriter<W> {
    pub fn new(writer: W) -> EventWriter<W> {
        let config = EmitterConfig::new()
            .line_separator("\n")
            .indent_string("\t")
            .perform_indent(true)
            .write_document_declaration(false)
            .normalize_empty_elements(true)
            .cdata_to_characters(true)
            .keep_element_names_stack(false)
            .autopad_comments(true);

        EventWriter {
            xml_writer: XmlEventWriter::new_with_config(writer, config),
            stack: Vec::new(),
            empty_namespace: Namespace::empty(),
        }
    }

    fn write_element_and_value(&mut self, name: &str, value: &str) -> Result<()> {
        self.start_element(name)?;
        self.write_value(value)?;
        self.end_element(name)?;
        Ok(())
    }

    fn start_element(&mut self, name: &str) -> Result<()> {
        self.xml_writer.write(WriteXmlEvent::StartElement {
            name: Name::local(name),
            attributes: Cow::Borrowed(&[]),
            namespace: Cow::Borrowed(&self.empty_namespace),
        })?;
        Ok(())
    }

    fn end_element(&mut self, name: &str) -> Result<()> {
        self.xml_writer.write(WriteXmlEvent::EndElement {
            name: Some(Name::local(name)),
        })?;
        Ok(())
    }

    fn write_value(&mut self, value: &str) -> Result<()> {
        self.xml_writer.write(WriteXmlEvent::Characters(value))?;
        Ok(())
    }

    fn maybe_end_plist(&mut self) -> Result<()> {
        // If there are no more open tags then write the </plist> element
        if self.stack.len() == 1 {
            // We didn't tell the xml_writer about the <plist> tag so we'll skip telling it
            // about the </plist> tag as well.
            self.xml_writer.inner_mut().write(b"\n</plist>")?;
            if let Some(Element::Root) = self.stack.pop() {
            } else {
                return Err(Error::InvalidData);
            }
        }
        Ok(())
    }

    pub fn write(&mut self, event: &PlistEvent) -> Result<()> {
        <Self as PlistEventWriter>::write(self, event)
    }
}

impl<W: Write> PlistEventWriter for EventWriter<W> {
    fn write(&mut self, event: &PlistEvent) -> Result<()> {
        match self.stack.pop() {
            Some(Element::Dictionary(DictionaryState::ExpectKey)) => {
                match *event {
                    PlistEvent::StringValue(ref value) => {
                        self.write_element_and_value("key", &*value)?;
                        self.stack
                            .push(Element::Dictionary(DictionaryState::ExpectValue));
                    }
                    PlistEvent::EndDictionary => {
                        self.end_element("dict")?;
                        // We might be closing the last tag here as well
                        self.maybe_end_plist()?;
                    }
                    _ => return Err(Error::InvalidData),
                };
                return Ok(());
            }
            Some(Element::Dictionary(DictionaryState::ExpectValue)) => self.stack
                .push(Element::Dictionary(DictionaryState::ExpectKey)),
            Some(other) => self.stack.push(other),
            None => {
                // Write prologue
                let prologue = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
"#;
                self.xml_writer.inner_mut().write(prologue.as_bytes())?;

                self.stack.push(Element::Root);
            }
        }

        match *event {
            PlistEvent::StartArray(_) => {
                self.start_element("array")?;
                self.stack.push(Element::Array);
            }
            PlistEvent::EndArray => {
                self.end_element("array")?;
                if let Some(Element::Array) = self.stack.pop() {
                } else {
                    return Err(Error::InvalidData);
                }
            }

            PlistEvent::StartDictionary(_) => {
                self.start_element("dict")?;
                self.stack
                    .push(Element::Dictionary(DictionaryState::ExpectKey));
            }
            PlistEvent::EndDictionary => return Err(Error::InvalidData),

            PlistEvent::BooleanValue(true) => {
                self.start_element("true")?;
                self.end_element("true")?;
            }
            PlistEvent::BooleanValue(false) => {
                self.start_element("false")?;
                self.end_element("false")?;
            }
            PlistEvent::DataValue(ref value) => {
                let base64_data = base64::encode_config(&value, base64::MIME);
                self.write_element_and_value("data", &base64_data)?;
            }
            PlistEvent::DateValue(ref value) => {
                self.write_element_and_value("date", &value.to_rfc3339())?
            }
            PlistEvent::IntegerValue(ref value) => {
                self.write_element_and_value("integer", &value.to_string())?
            }
            PlistEvent::RealValue(ref value) => {
                self.write_element_and_value("real", &value.to_string())?
            }
            PlistEvent::StringValue(ref value) => self.write_element_and_value("string", &*value)?,
        };

        self.maybe_end_plist()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use humantime::parse_rfc3339_weak;
    use std::io::Cursor;

    use super::*;

    #[test]
    fn streaming_parser() {
        use PlistEvent::*;

        let plist = &[
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
            DateValue(parse_rfc3339_weak("1981-05-16 11:32:06").unwrap().into()),
            EndDictionary,
        ];

        let mut cursor = Cursor::new(Vec::new());

        {
            let mut plist_w = EventWriter::new(&mut cursor);

            for item in plist {
                plist_w.write(item).unwrap();
            }
        }

        let comparison = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
\t<key>Author</key>
\t<string>William Shakespeare</string>
\t<key>Lines</key>
\t<array>
\t\t<string>It is a tale told by an idiot,</string>
\t\t<string>Full of sound and fury, signifying nothing.</string>
\t</array>
\t<key>Death</key>
\t<integer>1564</integer>
\t<key>Height</key>
\t<real>1.6</real>
\t<key>Data</key>
\t<data>AAAAvgAAAAMAAAAeAAAA</data>
\t<key>Birthdate</key>
\t<date>1981-05-16T11:32:06Z</date>
</dict>
</plist>";

        let s = String::from_utf8(cursor.into_inner()).unwrap();

        assert_eq!(s, comparison);
    }
}
