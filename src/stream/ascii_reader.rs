
/// Ascii property lists are used in legacy settings and only support four
/// datatypes: Array, Dictionary, String and Data.
/// See [Apple
/// Documentation](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/PropertyLists/OldStylePlists/OldStylePLists.htf
/// for more infos.

use std::{
    io::{Read, Seek, SeekFrom},
};
use crate::{
    error::{Error, ErrorKind, FilePosition},
    stream::Event,
    Date, Integer,
};

pub struct AsciiReader<R: Read + Seek> {
    reader: R,
    current_pos: u64,
}

impl<R: Read + Seek> AsciiReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            current_pos: 0, // FIXME: Track position!
        }
    }

    fn error(&self, kind: ErrorKind) -> Error {
        kind.with_byte_offset(self.current_pos)
    }

    /// Get a char without consuming it.
    fn peek(&mut self) -> Option<char> {
        // consume a char then rollback to previous position.
        let peeked = self.advance();
        let _ = self.reader.seek(SeekFrom::Current(-1));
        peeked
    }

    /// Get the next char without consuming it.
    fn peek_next(&mut self) -> Option<char> {
        let _ = self.reader.seek(SeekFrom::Current(1));
        let peeked = self.advance();
        let _ = self.reader.seek(SeekFrom::Current(-2));
        peeked
    }

    /// Consume the reader and return the next char.
    fn advance(&mut self) -> Option<char> {
        let mut buf: [u8; 1] = [0; 1];

        match self.reader.read(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    None
                } else {
                    let c =  buf[0] as char;
                    dbg!("Consuming: {}", c);
                    Some(c)
                }
            }
            Err(_) => None
        }
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
    fn unquoted_string_literal(&mut self, first: char) -> Result<Option<Event>, Error> {
        let mut acc = String::new();
        acc.push(first);

        while {
            match self.peek() {
                Some(c) => {
                    c != ' ' && c != '\r' && c != '\t' && c != ';'
                }
                None => false
            }
        } {
            // consuming the string itself
            match self.advance() {
                Some(c) => acc.push(c),
                None => return Err(self.error(ErrorKind::UnclosedString))
            };
        }

        Ok(Some(Event::String(acc)))
    }

    fn quoted_string_literal(&mut self) -> Result<Option<Event>, Error> {
        let mut acc = String::new();
        // Can the quote char be escaped?
        while {
            match self.peek() {
                Some(c) => c != '"',
                None => false
            }
        } {
            // consuming the string itself
            match self.advance() {
                Some(c) => acc.push(c),
                None => return Err(self.error(ErrorKind::UnclosedString))
            };
        }

        // Match the closing quote.
        match self.advance() {
            Some(c) => {
                if c == '"' {
                    Ok(Some(Event::String(acc)))
                } else {
                    Err(self.error(ErrorKind::UnclosedString))
                }
            }
            None => Err(self.error(ErrorKind::UnclosedString))
        }
    }

    /// Consumes the reader until it finds a valid Event
    /// Possible events for Ascii plists:
    //  - StartArray(Option<u64>),
    //  - StartDictionary(Option<u64>),
    //  - EndCollection,
    //  - Data(Vec<u8>),
    fn read_next(&mut self) -> Result<Option<Event>, Error> {
        while let Some(c) = self.advance() {
           match c {
                // Single char tokens
                '(' => return Ok(Some(Event::StartArray(None))),
                ')' => return Ok(Some(Event::EndCollection)),
                '{' => return Ok(Some(Event::StartDictionary(None))),
                '}' => return Ok(Some(Event::EndCollection)),
                'a'..='z' | 'A'..='Z' => return self.unquoted_string_literal(c),
                '"' => return self.quoted_string_literal(),
                ','| ';'| '=' => { /* consume these without doing anything */} ,
                ' ' | '\r' | '\t' | '\n' => { /* whitespace is not significant */},

                // Don't know what to do with these.
                _ => return Err(self.error(ErrorKind::UnexpectedChar))
            }
        }

        return Ok(None)
    }
}

impl<R: Read + Seek> Iterator for AsciiReader<R> {
    type Item = Result<Event, Error>;

    fn next(&mut self) -> Option<Result<Event, Error>> {
        match self.read_next() {
            Ok(Some(event)) => Some(Ok(event)),
            Err(err) => {
                // Mark the plist as finished
                // self.stack.clear();
                Some(Err(err))
            }
            Ok(None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, path::Path};

    use super::*;
    use crate::stream::Event::{self, *};

    #[test]
    fn streaming_animals() {
        let reader = File::open(&Path::new("./tests/data/ascii-animals.plist")).unwrap();
        let streaming_parser = AsciiReader::new(reader);
        let events: Vec<Event> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[
            StartDictionary(None),

            String("AnimalColors".to_owned()),
            StartDictionary(None),
            String("lamb".to_owned()), // key
            String("black".to_owned()),
            String("pig".to_owned()), // key
            String("pink".to_owned()),
            String("worm".to_owned()), // key
            String("pink".to_owned()),
            EndCollection,

            String("AnimalSmells".to_owned()),
            StartDictionary(None),
            String("lamb".to_owned()), // key
            String("lambish".to_owned()),
            String("pig".to_owned()), // key
            String("piggish".to_owned()),
            String("worm".to_owned()), // key
            String("wormy".to_owned()),
            EndCollection,

            String("AnimalSounds".to_owned()),
            StartDictionary(None),
            String("Lisa".to_owned()), // key
            String("Why is the worm talking like a lamb?".to_owned()),
            String("lamb".to_owned()), // key
            String("baa".to_owned()),
            String("pig".to_owned()), // key
            String("oink".to_owned()),
            String("worm".to_owned()), // key
            String("baa".to_owned()),
            EndCollection,

            EndCollection,
        ];

        assert_eq!(events, comparison);
    }
}