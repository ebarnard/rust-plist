use quick_xml::{events::Event as XmlEvent, Error as XmlReaderError, Reader as EventReader};
use std::io::{self, BufReader, Read};

use crate::{
    error::{Error, ErrorKind, FilePosition},
    stream::{Event, OwnedEvent},
    Date, Integer,
};

#[derive(Clone, PartialEq, Eq)]
struct ElmName(Box<[u8]>);

impl From<&[u8]> for ElmName {
    fn from(bytes: &[u8]) -> Self {
        ElmName(Box::from(bytes))
    }
}

impl AsRef<[u8]> for ElmName {
    fn as_ref(&self) -> &[u8] {
        &*self.0
    }
}

pub struct XmlReader<R: Read> {
    buffer: Vec<u8>,
    finished: bool,
    state: ReaderState<R>,
}

struct ReaderState<R: Read> {
    xml_reader: EventReader<BufReader<R>>,
    closed_element: Option<ElmName>,
    element_stack: Vec<ElmName>,
}

impl<R: Read> XmlReader<R> {
    pub fn new(reader: R) -> XmlReader<R> {
        let mut xml_reader = EventReader::from_reader(BufReader::new(reader));
        xml_reader.trim_text(true);

        XmlReader {
            buffer: Vec::new(),
            finished: false,
            state: ReaderState {
                xml_reader,
                closed_element: None,
                element_stack: Vec::new(),
            },
        }
    }
}

fn from_xml_error(err: XmlReaderError) -> ErrorKind {
    match err {
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
        XmlReaderError::UnexpectedEof(_) => ErrorKind::UnexpectedEof,
        XmlReaderError::Utf8(_) => ErrorKind::InvalidXmlUtf8,
        _ => ErrorKind::InvalidXmlSyntax,
    }
}

impl<R: Read> ReaderState<R> {
    fn xml_reader_pos(&self) -> FilePosition {
        let pos = self.xml_reader.buffer_position();
        FilePosition::Offset(pos as u64)
    }

    fn with_pos(&self, kind: ErrorKind) -> Error {
        kind.with_position(self.xml_reader_pos())
    }

    fn read_xml_event<'buf>(&mut self, buffer: &'buf mut Vec<u8>) -> Result<XmlEvent<'buf>, Error> {
        let event = self.xml_reader.read_event(buffer);
        let pos = self.xml_reader_pos();
        event.map_err(|err| from_xml_error(err).with_position(pos))
    }

    fn read_content(&mut self, buffer: &mut Vec<u8>) -> Result<String, Error> {
        loop {
            match self.read_xml_event(buffer)? {
                XmlEvent::Text(text) => {
                    let pos = self.xml_reader_pos();
                    let unespcaped = text
                        .unescaped()
                        .map_err(|err| from_xml_error(err).with_position(pos))?;
                    return String::from_utf8(unespcaped.to_vec())
                        .map_err(|_| ErrorKind::InvalidUtf8String.with_position(pos));
                }
                XmlEvent::End(element) => {
                    self.closed_element = Some(element.local_name().into());
                    return Ok("".to_owned());
                }
                XmlEvent::Eof => return Err(self.with_pos(ErrorKind::UnclosedXmlElement)),
                XmlEvent::Start(_) => return Err(self.with_pos(ErrorKind::UnexpectedXmlOpeningTag)),
                XmlEvent::PI(_)
                | XmlEvent::Empty(_)
                | XmlEvent::Comment(_)
                | XmlEvent::CData(_)
                | XmlEvent::Decl(_)
                | XmlEvent::DocType(_) => {
                    // skip
                }
            }
        }
    }

    fn read_next(&mut self, buffer: &mut Vec<u8>) -> Result<Option<OwnedEvent>, Error> {
        loop {
            if let Some(closed_name) = self.closed_element.take() {
                // Check the corrent element is being closed
                match self.element_stack.pop() {
                    Some(open_name) if closed_name == open_name => (),
                    Some(_open_name) => return Err(self.with_pos(ErrorKind::UnclosedXmlElement)),
                    None => return Err(self.with_pos(ErrorKind::UnpairedXmlClosingTag)),
                }

                match closed_name.as_ref() {
                    b"array" | b"dict" => return Ok(Some(Event::EndCollection)),
                    b"plist" | _ => {}
                }
            }

            match self.read_xml_event(buffer)? {
                XmlEvent::Start(name) => {
                    // Add the current element to the element stack
                    self.element_stack.push(name.local_name().into());

                    match name.local_name() {
                        b"plist" => {}
                        b"array" => return Ok(Some(Event::StartArray(None))),
                        b"dict" => return Ok(Some(Event::StartDictionary(None))),
                        b"key" => {
                            return Ok(Some(Event::String(self.read_content(buffer)?.into())))
                        }
                        b"data" => {
                            let mut encoded = self.read_content(buffer)?;
                            // Strip whitespace and line endings from input string
                            encoded.retain(|c| !c.is_ascii_whitespace());
                            let data = base64::decode(&encoded)
                                .map_err(|_| self.with_pos(ErrorKind::InvalidDataString))?;
                            return Ok(Some(Event::Data(data.into())));
                        }
                        b"date" => {
                            let s = self.read_content(buffer)?;
                            let date = Date::from_rfc3339(&s)
                                .map_err(|()| self.with_pos(ErrorKind::InvalidDateString))?;
                            return Ok(Some(Event::Date(date)));
                        }
                        b"integer" => {
                            let s = self.read_content(buffer)?;
                            match Integer::from_str(&s) {
                                Ok(i) => return Ok(Some(Event::Integer(i))),
                                Err(_) => {
                                    return Err(self.with_pos(ErrorKind::InvalidIntegerString))
                                }
                            }
                        }
                        b"real" => {
                            let s = self.read_content(buffer)?;
                            match s.parse() {
                                Ok(f) => return Ok(Some(Event::Real(f))),
                                Err(_) => return Err(self.with_pos(ErrorKind::InvalidRealString)),
                            }
                        }
                        b"string" => {
                            return Ok(Some(Event::String(self.read_content(buffer)?.into())))
                        }
                        _ => return Err(self.with_pos(ErrorKind::UnknownXmlElement)),
                    }
                }
                XmlEvent::Empty(name) => {
                    match name.local_name() {
                        b"true" => return Ok(Some(Event::Boolean(true))),
                        b"false" => return Ok(Some(Event::Boolean(false))),

                        b"array" | b"dict" => {
                            let owned_name = ElmName::from(name.local_name());
                            // Open and immediately close a collection element
                            self.element_stack.push(owned_name.clone());
                            self.closed_element = Some(owned_name);

                            match name.local_name() {
                                b"array" => return Ok(Some(Event::StartArray(None))),
                                b"dict" => return Ok(Some(Event::StartDictionary(None))),
                                _ => unreachable!(),
                            }
                        }

                        _ => return Err(self.with_pos(ErrorKind::UnknownXmlElement)),
                    }
                }
                XmlEvent::End(name) => {
                    // Check the corrent element is being closed
                    match self.element_stack.pop() {
                        Some(open_name) if name.local_name() == open_name.as_ref() => {}
                        Some(_open_name) => {
                            return Err(self.with_pos(ErrorKind::UnclosedXmlElement))
                        }
                        None => return Err(self.with_pos(ErrorKind::UnpairedXmlClosingTag)),
                    }

                    match name.local_name() {
                        b"array" | b"dict" => return Ok(Some(Event::EndCollection)),
                        b"plist" | _ => (),
                    }
                }
                XmlEvent::Eof if self.element_stack.is_empty() => return Ok(None),
                XmlEvent::Eof => return Err(self.with_pos(ErrorKind::UnclosedXmlElement)),
                XmlEvent::Text(_) => {
                    return Err(self.with_pos(ErrorKind::UnexpectedXmlCharactersExpectedElement))
                }
                XmlEvent::PI(_)
                | XmlEvent::Decl(_)
                | XmlEvent::DocType(_)
                | XmlEvent::CData(_)
                | XmlEvent::Comment(_) => {
                    // skip
                }
            }
        }
    }
}

impl<R: Read> Iterator for XmlReader<R> {
    type Item = Result<OwnedEvent, Error>;

    fn next(&mut self) -> Option<Result<OwnedEvent, Error>> {
        if self.finished {
            return None;
        }
        match self.state.read_next(&mut self.buffer) {
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
