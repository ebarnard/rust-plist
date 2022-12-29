use std::{error, fmt, io};

// use crate::{stream::Event, InvalidXmlDate};
use crate::stream::Event;

/// This type represents all possible errors that can occur when working with plist data.
#[derive(Debug)]
pub struct Error {
    inner: Box<ErrorImpl>,
}

#[derive(Debug)]
pub(crate) struct ErrorImpl {
    kind: ErrorKind,
    file_position: Option<FilePosition>,
}

#[derive(Debug)]
pub(crate) enum ErrorKind {
    UnexpectedEof,
    UnexpectedEndOfEventStream,
    UnexpectedEventType {
        #[allow(dead_code)]
        expected: EventKind,
        #[allow(dead_code)]
        found: EventKind,
    },

    // Xml format-specific errors
    UnclosedXmlElement,
    UnexpectedXmlCharactersExpectedElement,
    UnexpectedXmlOpeningTag,
    UnknownXmlElement,
    InvalidXmlSyntax,
    InvalidXmlUtf8,
    InvalidIntegerString,
    InvalidRealString,
    UidNotSupportedInXmlPlist,

    InvalidUtf8String,

    Io(io::Error),
    #[cfg(feature = "serde")]
    Serde(String),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FilePosition(pub(crate) u64);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum EventKind {
    StartDictionary,
    EndCollection,
    Boolean,
    Integer,
    Real,
    String,
    Uid,

    ValueOrStartCollection,
    DictionaryKeyOrEndCollection,
}

impl Error {
    /// Returns true if this error was caused by a failure to read or write bytes on an IO stream.
    pub fn is_io(&self) -> bool {
        self.as_io().is_some()
    }

    /// Returns true if this error was caused by prematurely reaching the end of the input data.
    pub fn is_eof(&self) -> bool {
        matches!(self.inner.kind, ErrorKind::UnexpectedEof)
    }

    /// Returns the underlying error if it was caused by a failure to read or write bytes on an IO
    /// stream.
    pub fn as_io(&self) -> Option<&io::Error> {
        if let ErrorKind::Io(err) = &self.inner.kind {
            Some(err)
        } else {
            None
        }
    }

    /// Returns the underlying error if it was caused by a failure to read or write bytes on an IO
    /// stream or `self` if it was not.
    pub fn into_io(self) -> Result<io::Error, Self> {
        if let ErrorKind::Io(err) = self.inner.kind {
            Ok(err)
        } else {
            Err(self)
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner.kind {
            ErrorKind::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(position) = &self.inner.file_position {
            write!(f, "{:?} ({})", &self.inner.kind, position)
        } else {
            fmt::Debug::fmt(&self.inner.kind, f)
        }
    }
}

impl fmt::Display for FilePosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "offset {}", self.0)
    }
}

impl ErrorKind {
    pub fn with_byte_offset(self, offset: u64) -> Error {
        self.with_position(FilePosition(offset))
    }

    pub fn with_position(self, pos: FilePosition) -> Error {
        Error {
            inner: Box::new(ErrorImpl {
                kind: self,
                file_position: Some(pos),
            }),
        }
    }

    pub fn without_position(self) -> Error {
        Error {
            inner: Box::new(ErrorImpl {
                kind: self,
                file_position: None,
            }),
        }
    }
}

impl EventKind {
    pub fn of_event(event: &Event) -> EventKind {
        match event {
            Event::StartDictionary(_) => EventKind::StartDictionary,
            Event::EndCollection => EventKind::EndCollection,
            Event::Boolean(_) => EventKind::Boolean,
            Event::Integer(_) => EventKind::Integer,
            Event::Real(_) => EventKind::Real,
            Event::String(_) => EventKind::String,
            Event::Uid(_) => EventKind::Uid,
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EventKind::StartDictionary => "StartDictionary",
            EventKind::EndCollection => "EndCollection",
            EventKind::Boolean => "Boolean",
            EventKind::Integer => "Integer",
            EventKind::Real => "Real",
            EventKind::String => "String",
            EventKind::Uid => "Uid",
            EventKind::ValueOrStartCollection => "value or start collection",
            EventKind::DictionaryKeyOrEndCollection => "dictionary key or end collection",
        }
        .fmt(f)
    }
}

pub(crate) fn from_io_without_position(err: io::Error) -> Error {
    ErrorKind::Io(err).without_position()
}

pub(crate) fn unexpected_event_type(expected: EventKind, found: &Event) -> Error {
    let found = EventKind::of_event(found);
    ErrorKind::UnexpectedEventType { expected, found }.without_position()
}
