/// Documentation:
/// - [Apple](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/PropertyLists/OldStylePlists/OldStylePLists.html)
/// - [GNUStep](http://wiki.gnustep.org/index.php?title=Property_Lists)
/// - [Binary Format](https://medium.com/@karaiskc/understanding-apples-binary-property-list-format-281e6da00dbd)
///

use std::{
    io::{Read, Seek, SeekFrom},
    str::FromStr,
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

    // One or two character tokens.
    Equal,
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




pub struct AsciiReader<R: Read + Seek> {
    reader: R,
    current_token: Vec<u8>,
    finished: bool,
    tokens: Vec<String>,
    had_errors: bool,

    /// start of the current lexeme
    lexeme_start: usize,
    current: usize,
    line: usize,
}

impl<R: Read + Seek> AsciiReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: reader,
            current_token: Vec::new(),
            finished: false,
            tokens: Vec::new(),
            had_errors: false,
            start: 0,
            current: 0,
            line: 1,
        }
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

    fn advance(&mut self) -> Option<char> {
        // self.source.chars().nth(self.current - 1)
        // self.current = self.current + 1;
        let mut buf: [u8; 1] = [0; 1];

        match self.reader.read(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    None
                } else {
                    Some(buf[0] as char)
                }
            }
            Err(_) => None
        }
    }

    fn add_token(&mut self, kind: TokenKind) {
        let reader_pos = self.reader.seek(SeekFrom::Current(0)).unwrap();
        let length = self.current - self.lexeme_start;

        let buf: Vec<u8> = Vec::with_capacity(length);
        self.reader.read(buf.as_mut_slice());

        // Goes back FIXME: NEEDED?
        self.reader.seek(SeekFrom::Start(reader_pos));

        // See add_token from rlox

        let token = Token::new(kind, text_slice.to_owned(), self.line);

        self.tokens.push(token);
    }

    fn scan_token(&mut self) {
        let c = match self.advance() {
            Some(c) => c,
            None => return
        };

        match c {
            // Single char tokens
            '(' => self.add_token(TokenKind::LeftParen,),
            ')' => self.add_token(TokenKind::RightParen),
            '{' => self.add_token(TokenKind::LeftBrace),
            '}' => self.add_token(TokenKind::RightBrace),
            ',' => self.add_token(TokenKind::Comma),
            ';' => self.add_token(TokenKind::SemiColon),
            '=' => self.add_token(TokenKind::Equal),

            // Single or two char(s) tokens
            '!' => {
                let token = if self.advance_if_matches('=') {
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                };
                self.add_token(token)
            },
            '=' => {
                let token = if self.advance_if_matches('=') {
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                };
                self.add_token(token)
            },

            // '/' can be a commented line.
            '/' => {
                if self.advance_if_matches('/') {
                    // consume the comment without doing anything with it.
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                } else {
                    self.add_token(TokenKind::Slash);
                }
            },

            // Eats whitespace
            ' ' | '\r' | '\t' => { /* Do Nothing */},

            '\n' => self.line = self.line + 1,

            // literals
            '"' => self.string_literal(),
            '0'..='9' => self.number_literal(),

            // identifer & keywords
            'a'..='z' | 'A'..='Z' | '_' => self.identifier(),

            // Don't know what to do with these.
            _ => self.error(self.line, "Unexpected character".to_owned())
        }
    }








    fn is_token_complete(&self) -> bool {
        false
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
    fn event_for_current_token(&self) -> Result<Option<Event>, Error> {

    }

    // fn advance(&mut self) {
    //     let mut buf = [0; 1];
    //     match self.reader.read(buf) {
    //         Ok(n) => {
    //             if n == 0 {
    //                 self.finished = true;
    //             } else {
    //                 self.current_token.append(buf[0]);
    //             }
    //         },
    //         Err(_e) =>  { /* FIXME: handle this case */ }
    //     }
    // }

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
