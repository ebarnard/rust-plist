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

    fn advance(&mut self) -> Option<char> {
        // self.source.chars().nth(self.current - 1)
        // self.current = self.current + 1;
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

    fn add_token(&mut self, kind: TokenKind) {
        let token = Token::new(
            kind,
            self.current_token.clone(),
            self.line
        );

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

            // Eats whitespace
            ' ' | '\r' | '\t' => { /* Do Nothing */},

            '\n' => self.line = self.line + 1,

            // literals
            '0'..='9' => self.number_literal(),
            'a'..='z' | 'A'..='Z' => self.unquoted_string_literal(),
            '"' => self.quoted_string_literal(),

            // Don't know what to do with these.
            _ => self.error(self.line, "Unexpected character".to_owned())
        }
    }

    fn number_literal(&mut self) {
        while self.peek().is_digit(10) {
            self.advance();
        }

        // Fractional part
        if self.peek() == '.' && self.peek_next().is_digit(10) {
            // consume '.'
            self.advance();

            while self.peek().is_digit(10) {
                self.advance();
            }
        }

        let double_value = self.current_token.parse::<f32>().unwrap();
        self.add_token(TokenKind::Number(double_value));
    }

    fn unquoted_string_literal(&mut self) {
        while self.peek() != ' ' && self.peek() != '\r' && self.peek() != '\t' && !self.is_at_end() {
            self.advance();
        }

        self.add_token(TokenKind::String(self.current_token.to_owned()));
    }

    fn quoted_string_literal(&mut self) {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line = self.line + 1;
            }

            self.advance();
        }

        if self.is_at_end() {
            self.error(self.line, "Unterminated string".to_owned());
            return
        }

        // closing quote
        self.advance();

        // +1/-1 because we don't want the quote
        let literal_length = self.current_token.len();
        let slice = &self.current_token[1..literal_length-1];
        self.add_token(TokenKind::String(slice.to_owned()));
    }

    fn error(&mut self, line: usize, message: String) {
        dbg!("ERROR line: {}, {}", line, message);
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
