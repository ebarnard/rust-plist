
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

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Single-character tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    SemiColon,
    Quote,
    Slash,
    Star,
    Equal,

    // One or two character tokens.
    LineComment,
    BlockCommentLeft,  // ie., /*
    BlockCommentRight, // ie., */

    // Literals.
    String(String),
    Number(f32),

    Eof,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: String, line: usize) -> Self {
        Token {
            kind,
            lexeme,
            line,
        }
    }
}

struct Scanner<R: Read + Seek> {
    reader: R,
    total_length: u64,
    tokens: Vec<Token>,
    /// start of the current lexeme
    current_token: String,
    current_token_start: u64,
    current_pos: u64,
    line: usize,
}

impl<R: Read + Seek> Scanner<R> {
    pub fn new(reader: R) -> Self {

        // Ouch
        let total_length = reader.seek(SeekFrom::End(0)).unwrap_or(0);
        reader.seek(SeekFrom::Start(0));

        Self {
            reader,
            total_length,
            tokens: Vec::new(),
            current_token: String::new(),
            current_token_start: 0,
            current_pos: 0,
            line: 1,
        }
    }

    fn error(&self, kind: ErrorKind) -> Error {
        // FIXME: Track position!
        kind.with_byte_offset(0)
    }

    fn is_at_end(&self) -> bool {
        self.current_pos >= self.total_length
    }

    /// Get the char without consuming it.
    fn peek(&self) -> char {
        let peeked = self.advance().unwrap_or('\0');
        self.reader.seek(SeekFrom::Current(-1));
        peeked
    }

    /// Get the next char without consuming it.
    fn peek_next(&self) -> char {
        self.reader.seek(SeekFrom::Current(1));
        let peeked = self.advance().unwrap_or('\0');
        self.reader.seek(SeekFrom::Current(-2));
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
                    let c = buf[0] as char;
                    self.current_token.push(c);
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
    fn unquoted_string_literal(&mut self) -> Result<Option<Event>, Error> {
        let mut acc = String::new();
        while self.peek() != ' ' && self.peek() != '\r' && self.peek() != '\t' && !self.is_at_end() {
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
        // FIXME: Get rid of is_at_end(). peek() should return an optional
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line = self.line + 1;
            }

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
    fn pull_next_event(&mut self) -> Result<Option<Event>, Error> {
        while let Some(c) = self.advance() {
           match c {
                // Single char tokens
                '(' => return Ok(Some(Event::StartArray(None))),
                ')' => return Ok(Some(Event::EndCollection)),
                '{' => return Ok(Some(Event::StartDictionary(None))),
                '}' => return Ok(Some(Event::EndCollection)),
                'a'..='z' | 'A'..='Z' => return self.unquoted_string_literal(),
                '"' => return self.quoted_string_literal(),
                '\n' => self.line = self.line + 1,
                ','| ';'| '=' => { /* consume these without doing anything */} ,
                ' ' | '\r' | '\t' => { /* whitespace is not significant */},

                // Don't know what to do with these.
                _ => return Err(self.error(ErrorKind::UnexpectedChar))
            }
        }

        return Ok(None)
    }
}

pub struct AsciiReader<R: Read + Seek> {
    reader: R,
    finished: bool,
    tokens: Vec<String>,
    had_errors: bool,
}

impl<R: Read + Seek> AsciiReader<R> {
    pub fn new(reader: R) -> Self {
        // TODO:
        // - initialize scanner
        // - make the pass
        // - pull from the scanner for parsing
        Self {
            reader: reader,
            current_token: Vec::new(),
            finished: false,
            tokens: Vec::new(),
            had_errors: false,

        }
    }

    // Possible events
    //     StartArray(Option<u64>),
    //     StartDictionary(Option<u64>),
    //     EndCollection,

    //     Boolean(bool),
    //     Data(Vec<u8>),
    //     Date(Date),
    //     Integer(Integer),
    //     Real(f64),
    //     String(String),
    //     Uid(Uid),

    fn read_next(&mut self) -> Result<Option<Event>, Error> {
        if self.finished {
            return Ok(None)
        } else {
            while !self.is_token_complete() {
                self.advance();
            }

            self.event_for_current_token()
        }
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
