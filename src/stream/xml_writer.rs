use quick_xml::{
    events::{BytesEnd, BytesStart, BytesText, Event as XmlEvent},
    Error as XmlWriterError, Writer as EventWriter,
};
use std::io::Write;

use crate::{
    error::{self, Error, ErrorKind, EventKind},
    stream::{Writer, XmlWriteOptions},
    Integer, Uid,
};

static XML_PROLOGUE: &[u8] = br#"<?xml version="1.0"?>
<plist version="1.0" gjver="2.0">
"#;

#[derive(PartialEq)]
enum Element {
    Dictionary,
    Array,
}

pub struct XmlWriter<W: Write> {
    xml_writer: EventWriter<W>,
    write_root_element: bool,
    started_plist: bool,
    stack: Vec<Element>,
    expecting_key: bool,
    pending_collection: Option<PendingCollection>,
    array_indexes: Vec<usize>,
}

enum PendingCollection {
    Array,
    Dictionary,
}

impl<W: Write> XmlWriter<W> {
    #[cfg(feature = "enable_unstable_features_that_may_break_with_minor_version_bumps")]
    pub fn new(writer: W) -> XmlWriter<W> {
        let opts = XmlWriteOptions::default();
        XmlWriter::new_with_options(writer, &opts)
    }

    pub fn new_with_options(writer: W, opts: &XmlWriteOptions) -> XmlWriter<W> {
        let xml_writer = if opts.indent_amount == 0 {
            EventWriter::new(writer)
        } else {
            EventWriter::new_with_indent(writer, opts.indent_char, opts.indent_amount)
        };

        XmlWriter {
            xml_writer,
            write_root_element: opts.root_element,
            started_plist: false,
            stack: Vec::new(),
            expecting_key: false,
            pending_collection: None,
            array_indexes: Vec::new(),
        }
    }

    #[cfg(feature = "enable_unstable_features_that_may_break_with_minor_version_bumps")]
    pub fn into_inner(self) -> W {
        self.xml_writer.into_inner()
    }

    fn write_element_and_value(&mut self, name: &str, value: &str) -> Result<(), Error> {
        self.start_element(name)?;
        self.write_value(value)?;
        self.end_element(name)?;
        Ok(())
    }

    fn start_element(&mut self, name: &str) -> Result<(), Error> {
        self.xml_writer
            .write_event(XmlEvent::Start(BytesStart::new(name)))?;
        Ok(())
    }

    fn end_element(&mut self, name: &str) -> Result<(), Error> {
        self.xml_writer
            .write_event(XmlEvent::End(BytesEnd::new(name)))?;
        Ok(())
    }

    fn write_value(&mut self, value: &str) -> Result<(), Error> {
        self.xml_writer
            .write_event(XmlEvent::Text(BytesText::new(value)))?;
        Ok(())
    }

    fn write_event<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        f: F,
    ) -> Result<(), Error> {
        if !self.started_plist {
            if self.write_root_element {
                self.xml_writer
                    .inner()
                    .write_all(XML_PROLOGUE)
                    .map_err(error::from_io_without_position)?;
            }

            self.started_plist = true;
        }

        f(self)?;

        // If there are no more open tags then write the </plist> element
        if self.stack.is_empty() {
            if self.write_root_element {
                // We didn't tell the xml_writer about the <plist> tag so we'll skip telling it
                // about the </plist> tag as well.
                self.xml_writer
                    .inner()
                    .write_all(b"\n</plist>")
                    .map_err(error::from_io_without_position)?;
            }

            self.xml_writer
                .inner()
                .flush()
                .map_err(error::from_io_without_position)?;
        }

        Ok(())
    }

    fn write_value_event<F: FnOnce(&mut Self) -> Result<(), Error>>(
        &mut self,
        event_kind: EventKind,
        f: F,
    ) -> Result<(), Error> {
        self.handle_pending_collection()?;
        self.write_event(|this| {
            if this.expecting_key {
                return Err(ErrorKind::UnexpectedEventType {
                    expected: EventKind::DictionaryKeyOrEndCollection,
                    found: event_kind,
                }
                .without_position());
            }
            f(this)?;
            this.expecting_key = this.stack.last() == Some(&Element::Dictionary);
            Ok(())
        })
    }

    fn handle_pending_collection(&mut self) -> Result<(), Error> {
        if let Some(PendingCollection::Array) = self.pending_collection {
            self.pending_collection = None;

            self.write_value_event(EventKind::StartArray, |this| {
                this.start_element("d")?;
                this.write_element_and_value("k", "_isArr")?;
                this.write_boolean(true)?;
                this.stack.push(Element::Array);
                this.array_indexes.push(0);
                Ok(())
            })
        } else if let Some(PendingCollection::Dictionary) = self.pending_collection {
            self.pending_collection = None;

            self.write_value_event(EventKind::StartDictionary, |this| {
                this.start_element(if !this.stack.is_empty() { "d" } else { "dict" })?;
                this.stack.push(Element::Dictionary);
                this.expecting_key = true;
                Ok(())
            })
        } else {
            Ok(())
        }
    }

    fn handle_array_index(&mut self) -> Result<(), Error> {
        if let Some(Element::Array) = self.stack.last() {
            let last_index = self.array_indexes.pop().unwrap_or(0);
            self.write_element_and_value("k", format!("k_{}", last_index).as_str())?;
            self.array_indexes.push(last_index + 1);
        };
        Ok(())
    }
}

impl<W: Write> Writer for XmlWriter<W> {
    fn write_start_array(&mut self, _len: Option<u64>) -> Result<(), Error> {
        self.handle_pending_collection()?;
        self.handle_array_index()?;
        self.pending_collection = Some(PendingCollection::Array);
        Ok(())
    }

    fn write_start_dictionary(&mut self, _len: Option<u64>) -> Result<(), Error> {
        self.handle_pending_collection()?;
        self.handle_array_index()?;
        self.pending_collection = Some(PendingCollection::Dictionary);
        Ok(())
    }

    fn write_end_collection(&mut self) -> Result<(), Error> {
        self.write_event(|this| {
            match this.pending_collection.take() {
                Some(PendingCollection::Array) => {
                    this.pending_collection = Some(PendingCollection::Array);
                    this.handle_pending_collection()?;
                    this.array_indexes.pop();
                }
                Some(PendingCollection::Dictionary) => {
                    this.xml_writer
                        .write_event(XmlEvent::Empty(BytesStart::new("d")))?;
                    this.expecting_key = this.stack.last() == Some(&Element::Dictionary);
                    return Ok(());
                }
                _ => {}
            };
            match (this.stack.pop(), this.expecting_key) {
                (Some(Element::Dictionary), true) | (Some(Element::Array), _) => {
                    this.end_element(if this.stack.is_empty() { "dict" } else { "d" })?;
                }
                (Some(Element::Dictionary), false) | (None, _) => {
                    return Err(ErrorKind::UnexpectedEventType {
                        expected: EventKind::ValueOrStartCollection,
                        found: EventKind::EndCollection,
                    }
                    .without_position());
                }
            }
            this.expecting_key = this.stack.last() == Some(&Element::Dictionary);
            Ok(())
        })
    }

    fn write_boolean(&mut self, value: bool) -> Result<(), Error> {
        self.handle_pending_collection()?;
        self.handle_array_index()?;
        self.write_value_event(EventKind::Boolean, |this| {
            let value = if value { "t" } else { "f" };
            Ok(this
                .xml_writer
                .write_event(XmlEvent::Empty(BytesStart::new(value)))?)
        })
    }

    fn write_integer(&mut self, value: Integer) -> Result<(), Error> {
        self.handle_pending_collection()?;
        self.handle_array_index()?;
        self.write_value_event(EventKind::Integer, |this| {
            this.write_element_and_value("i", &value.to_string())
        })
    }

    fn write_real(&mut self, value: f64) -> Result<(), Error> {
        self.handle_pending_collection()?;
        self.handle_array_index()?;
        self.write_value_event(EventKind::Real, |this| {
            this.write_element_and_value("r", &value.to_string())
        })
    }

    fn write_string(&mut self, value: &str) -> Result<(), Error> {
        self.handle_pending_collection()?;
        self.handle_array_index()?;
        self.write_event(|this| {
            if this.expecting_key {
                this.write_element_and_value("k", value)?;
                this.expecting_key = false;
            } else {
                this.write_element_and_value("s", value)?;
                this.expecting_key = this.stack.last() == Some(&Element::Dictionary);
            }
            Ok(())
        })
    }

    fn write_uid(&mut self, _value: Uid) -> Result<(), Error> {
        Err(ErrorKind::UidNotSupportedInXmlPlist.without_position())
    }
}

impl From<XmlWriterError> for Error {
    fn from(err: XmlWriterError) -> Self {
        match err {
            XmlWriterError::Io(err) => ErrorKind::Io(err).without_position(),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::stream::Event;

    #[test]
    fn streaming_parser() {
        let plist = &[
            Event::StartDictionary(None),
            Event::String("Author".into()),
            Event::String("William Shakespeare".into()),
            Event::String("Lines".into()),
            Event::StartArray(None),
            Event::String("It is a tale told by an idiot,".into()),
            Event::String("Full of sound and fury, signifying nothing.".into()),
            Event::EndCollection,
            Event::String("Death".into()),
            Event::Integer(1564.into()),
            Event::String("Height".into()),
            Event::Real(1.60),
            Event::String("Comment".into()),
            Event::String("2 < 3".into()), // make sure characters are escaped
            Event::String("BiggestNumber".into()),
            Event::Integer(18446744073709551615u64.into()),
            Event::String("SmallestNumber".into()),
            Event::Integer((-9223372036854775808i64).into()),
            Event::String("IsTrue".into()),
            Event::Boolean(true),
            Event::String("IsNotFalse".into()),
            Event::Boolean(false),
            Event::EndCollection,
        ];

        let expected = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
\t<k>Author</k>
\t<s>William Shakespeare</s>
\t<k>Lines</k>
\t<d>
\t\t<k>_isArr</k>
\t\t<t/>
\t\t<k>k_0</k>
\t\t<s>It is a tale told by an idiot,</s>
\t\t<k>k_1</k>
\t\t<s>Full of sound and fury, signifying nothing.</s>
\t</d>
\t<k>Death</k>
\t<i>1564</i>
\t<k>Height</k>
\t<r>1.6</r>
\t<k>Comment</k>
\t<s>2 &lt; 3</s>
\t<k>BiggestNumber</k>
\t<i>18446744073709551615</i>
\t<k>SmallestNumber</k>
\t<i>-9223372036854775808</i>
\t<k>IsTrue</k>
\t<t/>
\t<k>IsNotFalse</k>
\t<f/>
</dict>
</plist>";

        let actual = events_to_xml(plist, XmlWriteOptions::default());

        assert_eq!(actual, expected);
    }

    #[test]
    fn custom_indent_string() {
        let plist = &[
            Event::StartDictionary(None),
            Event::String("Lines".into()),
            Event::StartArray(None),
            Event::String("It is a tale told by an idiot,".into()),
            Event::String("Full of sound and fury, signifying nothing.".into()),
            Event::EndCollection,
            Event::EndCollection,
        ];

        let expected = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
...<k>Lines</k>
...<d>
......<k>_isArr</k>
......<t/>
......<k>k_0</k>
......<s>It is a tale told by an idiot,</s>
......<k>k_1</k>
......<s>Full of sound and fury, signifying nothing.</s>
...</d>
</dict>
</plist>";

        let actual = events_to_xml(plist, XmlWriteOptions::default().indent(b'.', 3));

        assert_eq!(actual, expected);
    }

    #[test]
    fn no_root() {
        let plist = &[
            Event::StartDictionary(None),
            Event::String("Lines".into()),
            Event::StartArray(None),
            Event::String("It is a tale told by an idiot,".into()),
            Event::String("Full of sound and fury, signifying nothing.".into()),
            Event::EndCollection,
            Event::EndCollection,
        ];

        let expected = "<dict>
\t<k>Lines</k>
\t<d>
\t\t<k>_isArr</k>
\t\t<t/>
\t\t<k>k_0</k>
\t\t<s>It is a tale told by an idiot,</s>
\t\t<k>k_1</k>
\t\t<s>Full of sound and fury, signifying nothing.</s>
\t</d>
</dict>";

        let actual = events_to_xml(plist, XmlWriteOptions::default().root_element(false));

        assert_eq!(actual, expected);
    }

    fn events_to_xml<'a>(
        events: impl IntoIterator<Item = &'a Event<'a>>,
        options: XmlWriteOptions,
    ) -> String {
        let mut cursor = Cursor::new(Vec::new());
        let mut writer = XmlWriter::new_with_options(&mut cursor, &options);
        for event in events {
            writer.write(event).unwrap();
        }
        String::from_utf8(cursor.into_inner()).unwrap()
    }
}
