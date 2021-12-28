use base64;
use std::{
    io::{self, Read, BufReader},
    str::FromStr,
};
use quick_xml::{
    Reader as EventReader,
    events::Event as XmlEvent,
    Error as XmlReaderError,
};

use crate::{
    error::{Error, ErrorKind, FilePosition},
    stream::{Event, OwnedEvent},
    Date, Integer,
};

mod xml_rs_stubs {
    /// Checks whether the given character is a white space character (`S`)
    /// as is defined by XML 1.1 specification, [section 2.3][1].
    ///
    /// [1]: http://www.w3.org/TR/2006/REC-xml11-20060816/#sec-common-syn
    pub fn is_whitespace_char(c: char) -> bool {
        match c {
            '\x20' | '\x09' | '\x0d' | '\x0a' => true,
            _ => false
        }
    }

    /// Checks whether the given string is compound only by white space
    /// characters (`S`) using the previous is_whitespace_char to check
    /// all characters of this string
    pub fn is_whitespace_str(s: &str) -> bool {
        s.chars().all(is_whitespace_char)
    }
}

use xml_rs_stubs::is_whitespace_str;

pub struct XmlReader<R: Read> {
    buffer: Vec<u8>,
    xml_reader: EventReader<BufReader<R>>,
    closed_element: Option<Box<[u8]>>,
    element_stack: Vec<Box<[u8]>>,
    finished: bool,
}

impl<R: Read> XmlReader<R> {
    pub fn new(reader: R) -> XmlReader<R> {
        // let config = ParserConfig::new()
        //     .trim_whitespace(false)
        //     .whitespace_to_characters(true)
        //     .cdata_to_characters(true)
        //     .ignore_comments(true)
        //     .coalesce_characters(true);

        let mut xml_reader = EventReader::from_reader(BufReader::new(reader));
        xml_reader.trim_text(true);

        XmlReader {
            buffer: Vec::new(),
            xml_reader,
            closed_element: None,
            element_stack: Vec::new(),
            finished: false,
        }
    }

    fn read_content(&mut self) -> Result<String, Error> {
        loop {
            match self.xml_reader.read_event(&mut self.buffer) {
                Ok(XmlEvent::Text(s)) => {
                    return String::from_utf8(s.unescaped().unwrap().to_vec())
                        .map_err(|_| ErrorKind::InvalidUtf8String.without_position())
                },
                Ok(XmlEvent::End(element)) => {
                    self.closed_element = Some(element.local_name().to_owned().into_boxed_slice());
                    return Ok("".to_owned());
                }
                Ok(XmlEvent::Eof) => {
                    return Err(self.with_pos(ErrorKind::UnclosedXmlElement))
                }
                Ok(XmlEvent::Start(_)) => {
                    return Err(self.with_pos(ErrorKind::UnexpectedXmlOpeningTag));
                }
                Ok(XmlEvent::PI(_)) => (),
                // Ok(XmlEvent::StartDocument { .. })
                // | Ok(XmlEvent::CData(_))
                // | Ok(XmlEvent::Comment(_))
                // | Ok(XmlEvent::Whitespace(_)) => {
                Ok(_) => {
                    unreachable!("parser does not output CData, Comment or Whitespace events");
                }
                Err(err) => return Err(from_xml_error(err)),
            }
        }
    }

    // fn next_event(&mut self) -> Result<XmlEvent, XmlReaderError> {
    //     self.xml_reader.read_event(&mut buffer)
    // }

    fn read_next(&mut self) -> Result<Option<OwnedEvent>, Error> {
        loop {
            if let Some(closed_name) = self.closed_element.take() {
                // Ok(XmlEvent::EndElement { name, .. }) => {
                    // Check the corrent element is being closed
                match self.element_stack.pop() {
                    Some(ref open_name) if &closed_name == open_name => (),
                    Some(ref _open_name) => {
                        return Err(self.with_pos(ErrorKind::UnclosedXmlElement))
                    }
                    None => return Err(self.with_pos(ErrorKind::UnpairedXmlClosingTag)),
                }

                match closed_name.as_ref() {
                    b"array" | b"dict" => return Ok(Some(Event::EndCollection)),
                    b"plist" | _ => (),
                }
                // }
            }

            match self.xml_reader.read_event(&mut self.buffer) {
                // Ok(XmlEvent::StartDocument { .. }) => {}
                Ok(XmlEvent::Start(name)) => {
                    // Add the current element to the element stack
                    self.element_stack.push(name.local_name().to_owned().into_boxed_slice());

                    match name.local_name() {
                        b"plist" => (),
                        b"array" => return Ok(Some(Event::StartArray(None))),
                        b"dict" => return Ok(Some(Event::StartDictionary(None))),
                        b"key" => return Ok(Some(Event::String(self.read_content()?.into()))),
                        b"data" => {
                            let mut s = self.read_content()?;
                            // Strip whitespace and line endings from input string
                            s.retain(|c| !c.is_ascii_whitespace());
                            let data = base64::decode(&s)
                                .map_err(|_| self.with_pos(ErrorKind::InvalidDataString))?;
                            return Ok(Some(Event::Data(data.into())));
                        }
                        b"date" => {
                            let s = self.read_content()?;
                            let date = Date::from_rfc3339(&s)
                                .map_err(|()| self.with_pos(ErrorKind::InvalidDateString))?;
                            return Ok(Some(Event::Date(date)));
                        }
                        b"integer" => {
                            let s = self.read_content()?;
                            match Integer::from_str(&s) {
                                Ok(i) => return Ok(Some(Event::Integer(i))),
                                Err(_) => {
                                    return Err(self.with_pos(ErrorKind::InvalidIntegerString))
                                }
                            }
                        }
                        b"real" => {
                            let s = self.read_content()?;
                            match f64::from_str(&s) {
                                Ok(f) => return Ok(Some(Event::Real(f))),
                                Err(_) => return Err(self.with_pos(ErrorKind::InvalidRealString)),
                            }
                        }
                        b"string" => return Ok(Some(Event::String(self.read_content()?.into()))),
                        _ => return Err(self.with_pos(ErrorKind::UnknownXmlElement)),
                    }
                }
                Ok(XmlEvent::Empty(name)) => {
                    match name.local_name() {
                        b"true" => return Ok(Some(Event::Boolean(true))),
                        b"false" => return Ok(Some(Event::Boolean(false))),

                        b"array" | b"dict" => {
                            let owned_name = name.local_name().to_owned().into_boxed_slice();
                            // Open and immediately close a collection element
                            self.element_stack.push(owned_name.clone());
                            self.closed_element = Some(owned_name);

                            match name.local_name() {
                                b"array" => return Ok(Some(Event::StartArray(None))),
                                b"dict" => return Ok(Some(Event::StartDictionary(None))),
                                _ => unreachable!(),
                            }
                        },

                        _ => return Err(self.with_pos(ErrorKind::UnknownXmlElement)),
                    }
                }
                Ok(XmlEvent::End(name)) => {
                    // Check the corrent element is being closed
                    match self.element_stack.pop() {
                        Some(ref open_name) if name.local_name() == open_name.as_ref() => (),
                        Some(ref _open_name) => {
                            return Err(self.with_pos(ErrorKind::UnclosedXmlElement))
                        }
                        None => return Err(self.with_pos(ErrorKind::UnpairedXmlClosingTag)),
                    }

                    match name.local_name() {
                        b"array" | b"dict" => return Ok(Some(Event::EndCollection)),
                        b"plist" | _ => (),
                    }
                }
                Ok(XmlEvent::Eof) => {
                    if self.element_stack.is_empty() {
                        return Ok(None);
                    } else {
                        return Err(self.with_pos(ErrorKind::UnclosedXmlElement));
                    }
                }
                Ok(XmlEvent::Text(c)) => {
                    // if !is_whitespace_str(&c) {
                        return Err(
                            self.with_pos(ErrorKind::UnexpectedXmlCharactersExpectedElement)
                        );
                    // }
                }
                Ok(XmlEvent::CData(_)) | Ok(XmlEvent::Comment(_)) /*| Ok(XmlEvent::Whitespace(_)) */ => {
                    unreachable!("parser does not output CData, Comment or Whitespace events")
                }
                Ok(XmlEvent::PI(_)) => (),
                Ok(XmlEvent::Decl(_)) | Ok(XmlEvent::DocType(_)) => (),
                Err(err) => return Err(from_xml_error(err)),
            }
        }
    }

    fn with_pos(&self, kind: ErrorKind) -> Error {
        kind.with_position(convert_xml_pos(self.xml_reader.buffer_position()))
    }
}

impl<R: Read> Iterator for XmlReader<R> {
    type Item = Result<OwnedEvent, Error>;

    fn next(&mut self) -> Option<Result<OwnedEvent, Error>> {
        if self.finished {
            None
        } else {
            match self.read_next() {
                Ok(Some(event)) => Some(Ok(event)),
                Ok(None) => {
                    self.finished = true;
                    None
                }
                Err(err) => {
                    self.finished = true;
                    Some(Err(err))
                }
            }
        }
    }
}

fn convert_xml_pos(pos: usize) -> FilePosition {
    // TODO: pos.row and pos.column counts from 0. what do we want to do?
    FilePosition::LineColumn(0, pos as u64)
}

fn from_xml_error(err: XmlReaderError) -> Error {
    let kind = match err {
        XmlReaderError::Io(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
            ErrorKind::UnexpectedEof
        }
        XmlReaderError::Io(err) => {
            let err = if let Some(code) = err.raw_os_error() {
                io::Error::from_raw_os_error(code)
            } else {
                io::Error::new(err.kind(), err.to_string())
            };
            ErrorKind::Io(err)
        }
        // XmlReaderError::Syntax(_) => ErrorKind::InvalidXmlSyntax,
        XmlReaderError::UnexpectedEof(_) => ErrorKind::UnexpectedEof,
        XmlReaderError::Utf8(_) => ErrorKind::InvalidXmlUtf8,
        _ => ErrorKind::InvalidXmlSyntax,
    };

    kind.with_position(convert_xml_pos(0))
}

#[cfg(test)]
mod tests {
    use std::{fs::File, path::Path};

    use super::*;
    use crate::stream::Event::{self, *};

    #[test]
    fn streaming_parser() {
        let reader = File::open(&Path::new("./tests/data/xml.plist")).unwrap();
        let streaming_parser = XmlReader::new(reader);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),
            String("Author".into()),
            String("William Shakespeare".into()),
            String("Lines".into()),
            StartArray(None),
            String("It is a tale told by an idiot,".into()),
            String("Full of sound and fury, signifying nothing.".into()),
            EndCollection,
            String("Death".into()),
            Integer(1564.into()),
            String("Height".into()),
            Real(1.60),
            String("Data".into()),
            Data(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0].into()),
            String("Birthdate".into()),
            Date(super::Date::from_rfc3339("1981-05-16T11:32:06Z").unwrap()),
            String("Blank".into()),
            String("".into()),
            String("BiggestNumber".into()),
            Integer(18446744073709551615u64.into()),
            String("SmallestNumber".into()),
            Integer((-9223372036854775808i64).into()),
            String("HexademicalNumber".into()),
            Integer(0xdead_beef_u64.into()),
            String("IsTrue".into()),
            Boolean(true),
            String("IsNotFalse".into()),
            Boolean(false),
            EndCollection,
        ];

        assert_eq!(events, comparison);
    }

    #[test]
    fn bad_data() {
        let reader = File::open(&Path::new("./tests/data/xml_error.plist")).unwrap();
        let streaming_parser = XmlReader::new(reader);
        let events: Vec<_> = streaming_parser.collect();

        assert!(events.last().unwrap().is_err());
    }
}
