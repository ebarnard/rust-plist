/// Ascii property lists are used in legacy settings and only support four
/// datatypes: Array, Dictionary, String and Data.
/// See [Apple
/// Documentation](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/PropertyLists/OldStylePlists/OldStylePLists.htf
/// for more infos.
/// However this reader also support Integers as first class datatype.
/// This reader will accept certain ill-formed ascii plist without complaining.
/// It does not check the integrity of the plist format.
use crate::{
    error::{Error, ErrorKind},
    stream::{Event, OwnedEvent},
    Integer,
};
use std::io::Read;

pub struct AsciiReader<R: Read> {
    reader: R,
    current_pos: u64,

    /// lookahead char to avoid backtracking.
    peeked_char: Option<u8>,
}

impl<R: Read> AsciiReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            current_pos: 0,
            peeked_char: None,
        }
    }

    fn error(&self, kind: ErrorKind) -> Error {
        kind.with_byte_offset(self.current_pos)
    }

    fn read_one(&mut self) -> Result<Option<u8>, Error> {
        let mut buf: [u8; 1] = [0; 1];
        match self.reader.read_exact(&mut buf) {
            Ok(()) => Ok(Some(buf[0])),
            Err(err) => {
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    Ok(None)
                } else {
                    Err(self.error(ErrorKind::IoReadError))
                }
            }
        }
    }

    /// Consume the reader and return the next char.
    fn advance(&mut self) -> Result<Option<u8>, Error> {
        let mut cur_char = self.peeked_char;
        self.peeked_char = self.read_one()?;

        // We need to read two chars to boot the process and fill the peeked
        // char.
        if self.current_pos == 0 {
            cur_char = self.peeked_char;
            self.peeked_char = self.read_one()?;
        }

        if cur_char.is_some() {
            self.current_pos += 1;
        }

        Ok(cur_char)
    }

    /// From Apple doc:
    ///
    /// > The quotation marks can be omitted if the string is composed strictly of alphanumeric
    /// > characters and contains no white space (numbers are handled as
    /// > strings in property lists). Though the property list format uses
    /// > ASCII for strings, note that Cocoa uses Unicode. Since string
    /// > encodings vary from region to region, this representation makes the
    /// > format fragile. You may see strings containing unreadable sequences of
    /// > ASCII characters; these are used to represent Unicode characters
    ///
    /// This function will naively try to convert the string to Integer.
    fn unquoted_string_literal(&mut self, first: u8) -> Result<Option<OwnedEvent>, Error> {
        let mut acc: Vec<u8> = Vec::new();
        acc.push(first);

        while {
            match self.peeked_char {
                Some(c) => {
                    c != b' ' && c != b')' && c != b'\r' && c != b'\t' && c != b';' && c != b','
                }
                None => false,
            }
        } {
            // consuming the string itself
            match self.advance()? {
                Some(c) => acc.push(c),
                None => return Err(self.error(ErrorKind::UnclosedString)),
            };
        }

        let string_literal =
            String::from_utf8(acc).map_err(|_e| self.error(ErrorKind::InvalidUtf8AsciiStream))?;

        // Not ideal but does the trick for now
        match Integer::from_str(&string_literal) {
            Ok(i) => Ok(Some(Event::Integer(i))),
            Err(_) => Ok(Some(Event::String(string_literal.into()))),
        }
    }

    fn quoted_string_literal(&mut self) -> Result<Option<OwnedEvent>, Error> {
        let mut acc: Vec<u8> = Vec::new();
        let mut cur_char = b'"';

        while {
            match self.peeked_char {
                // do not stop if the quote is escaped
                Some(c) => c != b'"' || cur_char == b'\\',
                None => false,
            }
        } {
            // consuming the string itself
            match self.advance()? {
                Some(c) => {
                    cur_char = c;
                    acc.push(c)
                }
                None => return Err(self.error(ErrorKind::UnclosedString)),
            };
        }

        // Match the closing quote.
        match self.advance()? {
            Some(c) => {
                if c as char == '"' {
                    let string_literal = String::from_utf8(acc)
                        .map_err(|_e| self.error(ErrorKind::InvalidUtf8AsciiStream))?;
                    Ok(Some(Event::String(string_literal.into())))
                } else {
                    Err(self.error(ErrorKind::UnclosedString))
                }
            }
            None => Err(self.error(ErrorKind::UnclosedString)),
        }
    }

    fn line_comment(&mut self) -> Result<(), Error> {
        // Consumes up to the end of the line.
        // There's no error in this a line comment can reach the EOF and there's
        // no forbidden chars in comments.
        while {
            match self.peeked_char {
                Some(c) => c != b'\n',
                None => false,
            }
        } {
            let _ = self.advance()?;
        }

        Ok(())
    }

    fn block_comment(&mut self) -> Result<(), Error> {
        let mut latest_consume = b' ';
        while {
            latest_consume != b'*'
                || match self.advance()? {
                    Some(c) => c != b'/',
                    None => false,
                }
        } {
            latest_consume = self.advance()?.unwrap_or(b' ');
        }

        Ok(())
    }

    /// Returns:
    /// - Some(string) if '/' was the first character of a string
    /// - None if '/' was the beginning of a comment.
    fn potential_comment(&mut self) -> Result<Option<OwnedEvent>, Error> {
        match self.peeked_char {
            Some(c) => match c {
                b'/' => self.line_comment().map(|_| None),
                b'*' => self.block_comment().map(|_| None),
                _ => self.unquoted_string_literal(c),
            },
            // EOF
            None => Err(self.error(ErrorKind::IncompleteComment)),
        }
    }

    /// Consumes the reader until it finds a valid Event
    /// Possible events for Ascii plists:
    ///  - StartArray(Option<u64>),
    ///  - StartDictionary(Option<u64>),
    ///  - EndCollection,
    ///  - Data(Vec<u8>),
    fn read_next(&mut self) -> Result<Option<OwnedEvent>, Error> {
        while let Some(c) = self.advance()? {
            match c {
                // Single char tokens
                b'(' => return Ok(Some(Event::StartArray(None))),
                b')' => return Ok(Some(Event::EndCollection)),
                b'{' => return Ok(Some(Event::StartDictionary(None))),
                b'}' => return Ok(Some(Event::EndCollection)),
                b'"' => return self.quoted_string_literal(),
                b'/' => {
                    match self.potential_comment() {
                        Ok(Some(event)) => return Ok(Some(event)),
                        Ok(None) => { /* Comment has been consumed */ }
                        Err(e) => return Err(e),
                    }
                }
                b',' | b';' | b'=' => { /* consume these without emitting anything */ }
                b' ' | b'\r' | b'\t' | b'\n' => { /* whitespace is not significant */ }
                _ => return self.unquoted_string_literal(c),
            }
        }

        Ok(None)
    }
}

impl<R: Read> Iterator for AsciiReader<R> {
    type Item = Result<OwnedEvent, Error>;

    fn next(&mut self) -> Option<Result<OwnedEvent, Error>> {
        match self.read_next() {
            Ok(Some(event)) => Some(Ok(event)),
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::Event::{self, *};
    use std::io::Cursor;
    use std::{fs::File, path::Path};

    #[test]
    fn empty_test() {
        let plist = "".to_owned();
        let cursor = Cursor::new(plist.as_bytes());
        let streaming_parser = AsciiReader::new(cursor);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();
        assert_eq!(events, &[]);
    }

    #[test]
    fn streaming_sample() {
        let reader = File::open(&Path::new("./tests/data/ascii-sample.plist")).unwrap();
        let streaming_parser = AsciiReader::new(reader);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),
            String("KeyName1".into()),
            String("Value1".into()),
            String("AnotherKeyName".into()),
            String("Value2".into()),
            String("Something".into()),
            StartArray(None),
            String("ArrayItem1".into()),
            String("ArrayItem2".into()),
            String("ArrayItem3".into()),
            EndCollection,
            String("Key4".into()),
            String("0.10".into()),
            String("KeyFive".into()),
            StartDictionary(None),
            String("Dictionary2Key1".into()),
            String("Something".into()),
            String("AnotherKey".into()),
            String("Somethingelse".into()),
            EndCollection,
            EndCollection,
        ];

        assert_eq!(events, comparison);
    }

    #[test]
    fn streaming_animals() {
        let reader = File::open(&Path::new("./tests/data/ascii-animals.plist")).unwrap();
        let streaming_parser = AsciiReader::new(reader);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),
            String("AnimalColors".into()),
            StartDictionary(None),
            String("lamb".into()), // key
            String("black".into()),
            String("pig".into()), // key
            String("pink".into()),
            String("worm".into()), // key
            String("pink".into()),
            EndCollection,
            String("AnimalSmells".into()),
            StartDictionary(None),
            String("lamb".into()), // key
            String("lambish".into()),
            String("pig".into()), // key
            String("piggish".into()),
            String("worm".into()), // key
            String("wormy".into()),
            EndCollection,
            String("AnimalSounds".into()),
            StartDictionary(None),
            String("Lisa".into()), // key
            String("Why is the worm talking like a lamb?".into()),
            String("lamb".into()), // key
            String("baa".into()),
            String("pig".into()), // key
            String("oink".into()),
            String("worm".into()), // key
            String("baa".into()),
            EndCollection,
            EndCollection,
        ];

        assert_eq!(events, comparison);
    }

    #[test]
    fn utf8_strings() {
        let plist = "{ names = (Léa, François, Żaklina, 王芳); }".to_owned();
        let cursor = Cursor::new(plist.as_bytes());
        let streaming_parser = AsciiReader::new(cursor);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),
            String("names".into()),
            StartArray(None),
            String("Léa".into()),
            String("François".into()),
            String("Żaklina".into()),
            String("王芳".into()),
            EndCollection,
            EndCollection,
        ];

        assert_eq!(events, comparison);
    }

    #[test]
    fn escaped_quotes_in_strings() {
        let plist = r#"{ key = "va\"lue" }"#;
        let cursor = Cursor::new(plist.as_bytes());
        let streaming_parser = AsciiReader::new(cursor);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),
            String("key".into()),
            String(r#"va\"lue"#.into()),
            EndCollection,
        ];

        assert_eq!(events, comparison);
    }

    #[test]
    fn integers_and_strings() {
        let plist = "{ name = James, age = 42 }".to_owned();
        let cursor = Cursor::new(plist.as_bytes());
        let streaming_parser = AsciiReader::new(cursor);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),
            String("name".into()),
            String("James".into()),
            String("age".into()),
            Integer(42.into()),
            EndCollection,
        ];

        assert_eq!(events, comparison);
    }

    #[test]
    fn netnewswire_pbxproj() {
        let reader = File::open(&Path::new("./tests/data/netnewswire.pbxproj")).unwrap();
        let streaming_parser = AsciiReader::new(reader);

        // Ensure that we don't fail when reading the file
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        assert!(!events.is_empty());
    }
}
