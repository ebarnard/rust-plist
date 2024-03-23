use base64::{engine::general_purpose::STANDARD as base64_standard, Engine};
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
        &self.0
    }
}

pub struct XmlReader<R: Read> {
    buffer: Vec<u8>,
    finished: bool,
    state: ReaderState<R>,
}

struct ReaderState<R: Read>(EventReader<BufReader<R>>);

impl<R: Read> XmlReader<R> {
    pub fn new(reader: R) -> XmlReader<R> {
        let mut xml_reader = EventReader::from_reader(BufReader::new(reader));
        xml_reader.trim_text(false);
        xml_reader.check_end_names(true);
        xml_reader.expand_empty_elements(true);

        XmlReader {
            buffer: Vec::new(),
            finished: false,
            state: ReaderState(xml_reader),
        }
    }
}

impl From<XmlReaderError> for ErrorKind {
    fn from(err: XmlReaderError) -> Self {
        match err {
            XmlReaderError::Io(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
                ErrorKind::UnexpectedEof
            }
            XmlReaderError::Io(err) => match std::sync::Arc::try_unwrap(err) {
                Ok(err) => ErrorKind::Io(err),
                Err(err) => ErrorKind::Io(std::io::Error::from(err.kind())),
            },
            XmlReaderError::UnexpectedEof(_) => ErrorKind::UnexpectedEof,
            XmlReaderError::NonDecodable(_) => ErrorKind::InvalidXmlUtf8,
            _ => ErrorKind::InvalidXmlSyntax,
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

impl<R: Read> ReaderState<R> {
    fn xml_reader_pos(&self) -> FilePosition {
        let pos = self.0.buffer_position();
        FilePosition(pos as u64)
    }

    fn with_pos(&self, kind: ErrorKind) -> Error {
        kind.with_position(self.xml_reader_pos())
    }

    fn read_xml_event<'buf>(&mut self, buffer: &'buf mut Vec<u8>) -> Result<XmlEvent<'buf>, Error> {
        let event = self.0.read_event_into(buffer);
        let pos = self.xml_reader_pos();
        event.map_err(|err| ErrorKind::from(err).with_position(pos))
    }

    fn read_content(&mut self, buffer: &mut Vec<u8>) -> Result<String, Error> {
        loop {
            match self.read_xml_event(buffer)? {
                XmlEvent::Text(text) => {
                    let unescaped = text
                        .unescape()
                        .map_err(|err| self.with_pos(ErrorKind::from(err)))?;
                    return String::from_utf8(unescaped.as_ref().into())
                        .map_err(|_| self.with_pos(ErrorKind::InvalidUtf8String));
                }
                XmlEvent::End(_) => {
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
            match self.read_xml_event(buffer)? {
                XmlEvent::Start(name) => {
                    match name.local_name().as_ref() {
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
                            let data = base64_standard
                                .decode(&encoded)
                                .map_err(|_| self.with_pos(ErrorKind::InvalidDataString))?;
                            return Ok(Some(Event::Data(data.into())));
                        }
                        b"date" => {
                            let s = self.read_content(buffer)?;
                            let date = Date::from_xml_format(&s)
                                .map_err(|_| self.with_pos(ErrorKind::InvalidDateString))?;
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
                        b"true" => return Ok(Some(Event::Boolean(true))),
                        b"false" => return Ok(Some(Event::Boolean(false))),
                        _ => return Err(self.with_pos(ErrorKind::UnknownXmlElement)),
                    }
                }
                XmlEvent::End(name) => match name.local_name().as_ref() {
                    b"array" | b"dict" => return Ok(Some(Event::EndCollection)),
                    _ => (),
                },
                XmlEvent::Eof => return Ok(None),
                XmlEvent::Text(text) => {
                    let unescaped = text
                        .unescape()
                        .map_err(|err| self.with_pos(ErrorKind::from(err)))?;

                    if !unescaped.chars().all(char::is_whitespace) {
                        return Err(
                            self.with_pos(ErrorKind::UnexpectedXmlCharactersExpectedElement)
                        );
                    }
                }
                XmlEvent::PI(_)
                | XmlEvent::Decl(_)
                | XmlEvent::DocType(_)
                | XmlEvent::CData(_)
                | XmlEvent::Comment(_)
                | XmlEvent::Empty(_) => {
                    // skip
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;
    use crate::stream::Event::*;

    #[test]
    fn streaming_parser() {
        let reader = File::open("./tests/data/xml.plist").unwrap();
        let streaming_parser = XmlReader::new(reader);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),
            String("Author".into()),
            String("William Shakespeare".into()),
            String("Lines".into()),
            StartArray(None),
            String("It is a tale told by an idiot,     ".into()),
            String("Full of sound and fury, signifying nothing.".into()),
            EndCollection,
            String("Death".into()),
            Integer(1564.into()),
            String("Height".into()),
            Real(1.60),
            String("Data".into()),
            Data(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0].into()),
            String("Birthdate".into()),
            Date(super::Date::from_xml_format("1981-05-16T11:32:06Z").unwrap()),
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
        let reader = File::open("./tests/data/xml_error.plist").unwrap();
        let streaming_parser = XmlReader::new(reader);
        let events: Vec<_> = streaming_parser.collect();

        assert!(events.last().unwrap().is_err());
    }
}
