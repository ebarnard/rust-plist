use base64;
use std::{io::Read, str::FromStr};
use xml_rs::reader::{EventReader, ParserConfig, XmlEvent};

use crate::{stream::Event, Date, Error, Integer};

pub struct XmlReader<R: Read> {
    xml_reader: EventReader<R>,
    queued_event: Option<XmlEvent>,
    element_stack: Vec<String>,
    finished: bool,
}

impl<R: Read> XmlReader<R> {
    pub fn new(reader: R) -> XmlReader<R> {
        let config = ParserConfig::new()
            .trim_whitespace(false)
            .whitespace_to_characters(true)
            .cdata_to_characters(true)
            .ignore_comments(true)
            .coalesce_characters(true);

        XmlReader {
            xml_reader: EventReader::new_with_config(reader, config),
            queued_event: None,
            element_stack: Vec::new(),
            finished: false,
        }
    }

    fn read_content<F>(&mut self, f: F) -> Result<Event, Error>
    where
        F: FnOnce(String) -> Result<Event, Error>,
    {
        match self.xml_reader.next() {
            Ok(XmlEvent::Characters(s)) => f(s),
            Ok(event @ XmlEvent::EndElement { .. }) => {
                self.queued_event = Some(event);
                f("".to_owned())
            }
            _ => Err(Error::InvalidData),
        }
    }

    fn next_event(&mut self) -> ::std::result::Result<XmlEvent, ()> {
        if let Some(event) = self.queued_event.take() {
            Ok(event)
        } else {
            self.xml_reader.next().map_err(|_| ())
        }
    }

    fn read_next(&mut self) -> Option<Result<Event, Error>> {
        loop {
            match self.next_event() {
                Ok(XmlEvent::StartElement { name, .. }) => {
                    // Add the current element to the element stack
                    self.element_stack.push(name.local_name.clone());

                    match &name.local_name[..] {
                        "plist" => (),
                        "array" => return Some(Ok(Event::StartArray(None))),
                        "dict" => return Some(Ok(Event::StartDictionary(None))),
                        "key" => return Some(self.read_content(|s| Ok(Event::String(s)))),
                        "true" => return Some(Ok(Event::Boolean(true))),
                        "false" => return Some(Ok(Event::Boolean(false))),
                        "data" => {
                            return Some(self.read_content(|mut s| {
                                // Strip whitespace and line endings from input string
                                s.retain(|c| !c.is_ascii_whitespace());
                                let data = base64::decode(&s).map_err(|_| Error::InvalidData)?;
                                Ok(Event::Data(data))
                            }));
                        }
                        "date" => {
                            return Some(self.read_content(|s| {
                                Ok(Event::Date(
                                    Date::from_rfc3339(&s).map_err(|()| Error::InvalidData)?,
                                ))
                            }));
                        }
                        "integer" => {
                            return Some(self.read_content(|s| match Integer::from_str(&s) {
                                Ok(i) => Ok(Event::Integer(i)),
                                Err(_) => Err(Error::InvalidData),
                            }));
                        }
                        "real" => {
                            return Some(self.read_content(|s| match f64::from_str(&s) {
                                Ok(f) => Ok(Event::Real(f)),
                                Err(_) => Err(Error::InvalidData),
                            }));
                        }
                        "string" => return Some(self.read_content(|s| Ok(Event::String(s)))),
                        _ => return Some(Err(Error::InvalidData)),
                    }
                }
                Ok(XmlEvent::EndElement { name, .. }) => {
                    // Check the corrent element is being closed
                    match self.element_stack.pop() {
                        Some(ref open_name) if &name.local_name == open_name => (),
                        Some(ref _open_name) => return Some(Err(Error::InvalidData)),
                        None => return Some(Err(Error::InvalidData)),
                    }

                    match &name.local_name[..] {
                        "array" | "dict" => return Some(Ok(Event::EndCollection)),
                        "plist" => (),
                        _ => (),
                    }
                }
                Ok(XmlEvent::EndDocument) => {
                    if self.element_stack.is_empty() {
                        return None;
                    } else {
                        return Some(Err(Error::UnexpectedEof));
                    }
                }
                Err(_) => return Some(Err(Error::InvalidData)),
                _ => (),
            }
        }
    }
}

impl<R: Read> Iterator for XmlReader<R> {
    type Item = Result<Event, Error>;

    fn next(&mut self) -> Option<Result<Event, Error>> {
        if self.finished {
            None
        } else {
            match self.read_next() {
                Some(Ok(event)) => Some(Ok(event)),
                Some(Err(err)) => {
                    self.finished = true;
                    Some(Err(err))
                }
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
    use humantime::parse_rfc3339_weak;
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
            String("Author".to_owned()),
            String("William Shakespeare".to_owned()),
            String("Lines".to_owned()),
            StartArray(None),
            String("It is a tale told by an idiot,".to_owned()),
            String("Full of sound and fury, signifying nothing.".to_owned()),
            EndCollection,
            String("Death".to_owned()),
            Integer(1564.into()),
            String("Height".to_owned()),
            Real(1.60),
            String("Data".to_owned()),
            Data(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0]),
            String("Birthdate".to_owned()),
            Date(parse_rfc3339_weak("1981-05-16 11:32:06").unwrap().into()),
            String("Blank".to_owned()),
            String("".to_owned()),
            String("BiggestNumber".to_owned()),
            Integer(18446744073709551615u64.into()),
            String("SmallestNumber".to_owned()),
            Integer((-9223372036854775808i64).into()),
            String("HexademicalNumber".to_owned()),
            Integer(0xdead_beef_u64.into()),
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
