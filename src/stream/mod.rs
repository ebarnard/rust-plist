//! An abstraction of a plist file as a stream of events. Used to support multiple encodings.

mod binary_reader;
pub use self::binary_reader::BinaryReader;

mod xml_reader;
pub use self::xml_reader::XmlReader;

mod xml_writer;
pub use self::xml_writer::XmlWriter;

use std::io::{Read, Seek, SeekFrom};
use std::vec;
use {Date, Error, Integer, Value};

/// An encoding of a plist as a flat structure.
///
/// Output by the event readers.
///
/// Dictionary keys and values are represented as pairs of values e.g.:
///
/// ```ignore rust
/// StartDictionary
/// String("Height") // Key
/// Real(181.2)      // Value
/// String("Age")    // Key
/// Integer(28)      // Value
/// EndDictionary
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    // While the length of an array or dict cannot be feasably greater than max(usize) this better
    // conveys the concept of an effectively unbounded event stream.
    StartArray(Option<u64>),
    EndArray,

    StartDictionary(Option<u64>),
    EndDictionary,

    Boolean(bool),
    Data(Vec<u8>),
    Date(Date),
    Integer(Integer),
    Real(f64),
    String(String),

    #[doc(hidden)]
    __Nonexhaustive,
}

/// An `Event` stream returned by `Value::into_events`.
pub struct IntoEvents {
    events: vec::IntoIter<Event>,
}

impl IntoEvents {
    pub(crate) fn new(value: Value) -> IntoEvents {
        let mut events = Vec::new();
        IntoEvents::new_inner(value, &mut events);
        IntoEvents {
            events: events.into_iter(),
        }
    }

    fn new_inner(value: Value, events: &mut Vec<Event>) {
        match value {
            Value::Array(array) => {
                events.push(Event::StartArray(Some(array.len() as u64)));
                for value in array {
                    IntoEvents::new_inner(value, events);
                }
                events.push(Event::EndArray);
            }
            Value::Dictionary(dict) => {
                events.push(Event::StartDictionary(Some(dict.len() as u64)));
                for (key, value) in dict {
                    events.push(Event::String(key));
                    IntoEvents::new_inner(value, events);
                }
                events.push(Event::EndDictionary);
            }
            Value::Boolean(value) => events.push(Event::Boolean(value)),
            Value::Data(value) => events.push(Event::Data(value)),
            Value::Date(value) => events.push(Event::Date(value)),
            Value::Real(value) => events.push(Event::Real(value)),
            Value::Integer(value) => events.push(Event::Integer(value)),
            Value::String(value) => events.push(Event::String(value)),
            Value::__Nonexhaustive => unreachable!(),
        }
    }
}

impl Iterator for IntoEvents {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.events.next()
    }
}

pub struct Reader<R: Read + Seek>(ReaderInner<R>);

enum ReaderInner<R: Read + Seek> {
    Uninitialized(Option<R>),
    Xml(XmlReader<R>),
    Binary(BinaryReader<R>),
}

impl<R: Read + Seek> Reader<R> {
    pub fn new(reader: R) -> Reader<R> {
        Reader(ReaderInner::Uninitialized(Some(reader)))
    }

    fn is_binary(reader: &mut R) -> Result<bool, Error> {
        reader.seek(SeekFrom::Start(0))?;
        let mut magic = [0; 8];
        reader.read_exact(&mut magic)?;
        reader.seek(SeekFrom::Start(0))?;

        Ok(&magic == b"bplist00")
    }
}

impl<R: Read + Seek> Iterator for Reader<R> {
    type Item = Result<Event, Error>;

    fn next(&mut self) -> Option<Result<Event, Error>> {
        let mut reader = match self.0 {
            ReaderInner::Xml(ref mut parser) => return parser.next(),
            ReaderInner::Binary(ref mut parser) => return parser.next(),
            ReaderInner::Uninitialized(ref mut reader) => reader.take().unwrap(),
        };

        let event_reader = match Reader::is_binary(&mut reader) {
            Ok(true) => ReaderInner::Binary(BinaryReader::new(reader)),
            Ok(false) => ReaderInner::Xml(XmlReader::new(reader)),
            Err(err) => {
                ::std::mem::replace(&mut self.0, ReaderInner::Uninitialized(Some(reader)));
                return Some(Err(err));
            }
        };

        ::std::mem::replace(&mut self.0, event_reader);

        self.next()
    }
}

/// Supports writing event streams in different plist encodings.
pub trait Writer: private::Sealed {
    fn write(&mut self, event: &Event) -> Result<(), Error> {
        match event {
            Event::StartArray(len) => self.write_start_array(*len),
            Event::EndArray => self.write_end_array(),
            Event::StartDictionary(len) => self.write_start_dictionary(*len),
            Event::EndDictionary => self.write_end_dictionary(),
            Event::Boolean(value) => self.write_boolean_value(*value),
            Event::Data(value) => self.write_data_value(value),
            Event::Date(value) => self.write_date_value(*value),
            Event::Integer(value) => self.write_integer_value(*value),
            Event::Real(value) => self.write_real_value(*value),
            Event::String(value) => self.write_string_value(value),
            Event::__Nonexhaustive => unreachable!(),
        }
    }

    fn write_start_array(&mut self, len: Option<u64>) -> Result<(), Error>;
    fn write_end_array(&mut self) -> Result<(), Error>;

    fn write_start_dictionary(&mut self, len: Option<u64>) -> Result<(), Error>;
    fn write_end_dictionary(&mut self) -> Result<(), Error>;

    fn write_boolean_value(&mut self, value: bool) -> Result<(), Error>;
    fn write_data_value(&mut self, value: &[u8]) -> Result<(), Error>;
    fn write_date_value(&mut self, value: Date) -> Result<(), Error>;
    fn write_integer_value(&mut self, value: Integer) -> Result<(), Error>;
    fn write_real_value(&mut self, value: f64) -> Result<(), Error>;
    fn write_string_value(&mut self, value: &str) -> Result<(), Error>;
}

#[doc(hidden)]
pub struct VecWriter {
    events: Vec<Event>,
}

impl VecWriter {
    pub fn new() -> VecWriter {
        VecWriter { events: Vec::new() }
    }

    pub fn into_inner(self) -> Vec<Event> {
        self.events
    }
}

impl Writer for VecWriter {
    fn write_start_array(&mut self, len: Option<u64>) -> Result<(), Error> {
        self.events.push(Event::StartArray(len));
        Ok(())
    }

    fn write_end_array(&mut self) -> Result<(), Error> {
        self.events.push(Event::EndArray);
        Ok(())
    }

    fn write_start_dictionary(&mut self, len: Option<u64>) -> Result<(), Error> {
        self.events.push(Event::StartDictionary(len));
        Ok(())
    }

    fn write_end_dictionary(&mut self) -> Result<(), Error> {
        self.events.push(Event::EndDictionary);
        Ok(())
    }

    fn write_boolean_value(&mut self, value: bool) -> Result<(), Error> {
        self.events.push(Event::Boolean(value));
        Ok(())
    }

    fn write_data_value(&mut self, value: &[u8]) -> Result<(), Error> {
        self.events.push(Event::Data(value.to_owned()));
        Ok(())
    }

    fn write_date_value(&mut self, value: Date) -> Result<(), Error> {
        self.events.push(Event::Date(value));
        Ok(())
    }

    fn write_integer_value(&mut self, value: Integer) -> Result<(), Error> {
        self.events.push(Event::Integer(value));
        Ok(())
    }

    fn write_real_value(&mut self, value: f64) -> Result<(), Error> {
        self.events.push(Event::Real(value));
        Ok(())
    }

    fn write_string_value(&mut self, value: &str) -> Result<(), Error> {
        self.events.push(Event::String(value.to_owned()));
        Ok(())
    }
}

mod private {
    use std::io::Write;

    pub trait Sealed {}

    impl<W: Write> Sealed for super::XmlWriter<W> {}
    impl Sealed for super::VecWriter {}
}
