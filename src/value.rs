use std::collections::BTreeMap;
use std::io::{Read, Seek};

use events::{Event, Reader};
use {u64_option_to_usize, Date, Error};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Array(Vec<Value>),
    Dictionary(BTreeMap<String, Value>),
    Boolean(bool),
    Data(Vec<u8>),
    Date(Date),
    Real(f64),
    Integer(i64),
    String(String),
}

impl Value {
    pub fn read<R: Read + Seek>(reader: R) -> Result<Value, Error> {
        let reader = Reader::new(reader);
        Value::from_events(reader)
    }

    pub fn from_events<T>(events: T) -> Result<Value, Error>
    where
        T: IntoIterator<Item = Result<Event, Error>>,
    {
        Builder::new(events.into_iter()).build()
    }

    pub fn into_events(self) -> Vec<Event> {
        let mut events = Vec::new();
        self.into_events_inner(&mut events);
        events
    }

    fn into_events_inner(self, events: &mut Vec<Event>) {
        match self {
            Value::Array(array) => {
                events.push(Event::StartArray(Some(array.len() as u64)));
                for value in array {
                    value.into_events_inner(events);
                }
                events.push(Event::EndArray);
            }
            Value::Dictionary(dict) => {
                events.push(Event::StartDictionary(Some(dict.len() as u64)));
                for (key, value) in dict {
                    events.push(Event::StringValue(key));
                    value.into_events_inner(events);
                }
                events.push(Event::EndDictionary);
            }
            Value::Boolean(value) => events.push(Event::BooleanValue(value)),
            Value::Data(value) => events.push(Event::DataValue(value)),
            Value::Date(value) => events.push(Event::DateValue(value)),
            Value::Real(value) => events.push(Event::RealValue(value)),
            Value::Integer(value) => events.push(Event::IntegerValue(value)),
            Value::String(value) => events.push(Event::StringValue(value)),
        }
    }

    /// If the `Value` is an Array, returns the associated Vec.
    /// Returns None otherwise.
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match *self {
            Value::Array(ref array) => Some(array),
            _ => None,
        }
    }

    /// If the `Value` is an Array, returns the associated mutable Vec.
    /// Returns None otherwise.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match *self {
            Value::Array(ref mut array) => Some(array),
            _ => None,
        }
    }

    /// If the `Value` is a Dictionary, returns the associated BTreeMap.
    /// Returns None otherwise.
    pub fn as_dictionary(&self) -> Option<&BTreeMap<String, Value>> {
        match *self {
            Value::Dictionary(ref map) => Some(map),
            _ => None,
        }
    }

    /// If the `Value` is a Dictionary, returns the associated mutable BTreeMap.
    /// Returns None otherwise.
    pub fn as_dictionary_mut(&mut self) -> Option<&mut BTreeMap<String, Value>> {
        match *self {
            Value::Dictionary(ref mut map) => Some(map),
            _ => None,
        }
    }

    /// If the `Value` is a Boolean, returns the associated bool.
    /// Returns None otherwise.
    pub fn as_boolean(&self) -> Option<bool> {
        match *self {
            Value::Boolean(v) => Some(v),
            _ => None,
        }
    }

    /// If the `Value` is a Data, returns the underlying Vec.
    /// Returns None otherwise.
    ///
    /// This method consumes the `Value`. If this is not desired, please use
    /// `as_data` method.
    pub fn into_data(self) -> Option<Vec<u8>> {
        match self {
            Value::Data(data) => Some(data),
            _ => None,
        }
    }

    /// If the `Value` is a Data, returns the associated Vec.
    /// Returns None otherwise.
    pub fn as_data(&self) -> Option<&[u8]> {
        match *self {
            Value::Data(ref data) => Some(data),
            _ => None,
        }
    }

    /// If the `Value` is a Date, returns the associated DateTime.
    /// Returns None otherwise.
    pub fn as_date(&self) -> Option<&Date> {
        match *self {
            Value::Date(ref date) => Some(date),
            _ => None,
        }
    }

    /// If the `Value` is a Real, returns the associated f64.
    /// Returns None otherwise.
    pub fn as_real(&self) -> Option<f64> {
        match *self {
            Value::Real(v) => Some(v),
            _ => None,
        }
    }

    /// If the `Value` is an Integer, returns the associated i64.
    /// Returns None otherwise.
    pub fn as_integer(&self) -> Option<i64> {
        match *self {
            Value::Integer(v) => Some(v),
            _ => None,
        }
    }

    /// If the `Value` is a String, returns the underlying String.
    /// Returns None otherwise.
    ///
    /// This method consumes the `Value`. If this is not desired, please use
    /// `as_string` method.
    pub fn into_string(self) -> Option<String> {
        match self {
            Value::String(v) => Some(v),
            _ => None,
        }
    }

    /// If the `Value` is a String, returns the associated str.
    /// Returns None otherwise.
    pub fn as_string(&self) -> Option<&str> {
        match *self {
            Value::String(ref v) => Some(v),
            _ => None,
        }
    }
}

impl From<Vec<Value>> for Value {
    fn from(from: Vec<Value>) -> Value {
        Value::Array(from)
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(from: BTreeMap<String, Value>) -> Value {
        Value::Dictionary(from)
    }
}

impl From<bool> for Value {
    fn from(from: bool) -> Value {
        Value::Boolean(from)
    }
}

impl<'a> From<&'a bool> for Value {
    fn from(from: &'a bool) -> Value {
        Value::Boolean(*from)
    }
}

impl From<Date> for Value {
    fn from(from: Date) -> Value {
        Value::Date(from)
    }
}

impl<'a> From<&'a Date> for Value {
    fn from(from: &'a Date) -> Value {
        Value::Date(from.clone())
    }
}

impl From<f64> for Value {
    fn from(from: f64) -> Value {
        Value::Real(from)
    }
}

impl From<f32> for Value {
    fn from(from: f32) -> Value {
        Value::Real(from.into())
    }
}

impl From<i64> for Value {
    fn from(from: i64) -> Value {
        Value::Integer(from)
    }
}

impl From<i32> for Value {
    fn from(from: i32) -> Value {
        Value::Integer(from.into())
    }
}

impl From<i16> for Value {
    fn from(from: i16) -> Value {
        Value::Integer(from.into())
    }
}

impl From<i8> for Value {
    fn from(from: i8) -> Value {
        Value::Integer(from.into())
    }
}

impl From<u32> for Value {
    fn from(from: u32) -> Value {
        Value::Integer(from.into())
    }
}

impl From<u16> for Value {
    fn from(from: u16) -> Value {
        Value::Integer(from.into())
    }
}

impl From<u8> for Value {
    fn from(from: u8) -> Value {
        Value::Integer(from.into())
    }
}

impl<'a> From<&'a f64> for Value {
    fn from(from: &'a f64) -> Value {
        Value::Real(*from)
    }
}

impl<'a> From<&'a f32> for Value {
    fn from(from: &'a f32) -> Value {
        Value::Real((*from).into())
    }
}

impl<'a> From<&'a i64> for Value {
    fn from(from: &'a i64) -> Value {
        Value::Integer(*from)
    }
}

impl<'a> From<&'a i32> for Value {
    fn from(from: &'a i32) -> Value {
        Value::Integer((*from).into())
    }
}

impl<'a> From<&'a i16> for Value {
    fn from(from: &'a i16) -> Value {
        Value::Integer((*from).into())
    }
}

impl<'a> From<&'a i8> for Value {
    fn from(from: &'a i8) -> Value {
        Value::Integer((*from).into())
    }
}

impl<'a> From<&'a u32> for Value {
    fn from(from: &'a u32) -> Value {
        Value::Integer((*from).into())
    }
}

impl<'a> From<&'a u16> for Value {
    fn from(from: &'a u16) -> Value {
        Value::Integer((*from).into())
    }
}

impl<'a> From<&'a u8> for Value {
    fn from(from: &'a u8) -> Value {
        Value::Integer((*from).into())
    }
}

impl From<String> for Value {
    fn from(from: String) -> Value {
        Value::String(from)
    }
}

impl<'a> From<&'a str> for Value {
    fn from(from: &'a str) -> Value {
        Value::String(from.into())
    }
}

struct Builder<T> {
    stream: T,
    token: Option<Event>,
}

impl<T: Iterator<Item = Result<Event, Error>>> Builder<T> {
    fn new(stream: T) -> Builder<T> {
        Builder {
            stream,
            token: None,
        }
    }

    fn build(mut self) -> Result<Value, Error> {
        self.bump()?;
        let plist = self.build_value()?;

        // Ensure the stream has been fully consumed
        self.bump()?;
        match self.token {
            None => Ok(plist),
            _ => Err(Error::InvalidData),
        }
    }

    fn bump(&mut self) -> Result<(), Error> {
        self.token = match self.stream.next() {
            Some(Ok(token)) => Some(token),
            Some(Err(err)) => return Err(err),
            None => None,
        };
        Ok(())
    }

    fn build_value(&mut self) -> Result<Value, Error> {
        match self.token.take() {
            Some(Event::StartArray(len)) => Ok(Value::Array(self.build_array(len)?)),
            Some(Event::StartDictionary(len)) => Ok(Value::Dictionary(self.build_dict(len)?)),

            Some(Event::BooleanValue(b)) => Ok(Value::Boolean(b)),
            Some(Event::DataValue(d)) => Ok(Value::Data(d)),
            Some(Event::DateValue(d)) => Ok(Value::Date(d)),
            Some(Event::IntegerValue(i)) => Ok(Value::Integer(i)),
            Some(Event::RealValue(f)) => Ok(Value::Real(f)),
            Some(Event::StringValue(s)) => Ok(Value::String(s)),

            Some(Event::EndArray) => Err(Error::InvalidData),
            Some(Event::EndDictionary) => Err(Error::InvalidData),

            // The stream should not have ended here
            None => Err(Error::InvalidData),
        }
    }

    fn build_array(&mut self, len: Option<u64>) -> Result<Vec<Value>, Error> {
        let len = u64_option_to_usize(len)?;
        let mut values = match len {
            Some(len) => Vec::with_capacity(len),
            None => Vec::new(),
        };

        loop {
            self.bump()?;
            if let Some(Event::EndArray) = self.token {
                self.token.take();
                return Ok(values);
            }
            values.push(self.build_value()?);
        }
    }

    fn build_dict(&mut self, _len: Option<u64>) -> Result<BTreeMap<String, Value>, Error> {
        let mut values = BTreeMap::new();

        loop {
            self.bump()?;
            match self.token.take() {
                Some(Event::EndDictionary) => return Ok(values),
                Some(Event::StringValue(s)) => {
                    self.bump()?;
                    values.insert(s, self.build_value()?);
                }
                _ => {
                    // Only string keys are supported in plists
                    return Err(Error::InvalidData);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::SystemTime;

    use super::*;
    use events::Event::*;
    use {Date, Value};

    #[test]
    fn value_accessors() {
        let vec = vec![Value::Real(0.0)];
        let mut array = Value::Array(vec.clone());
        assert_eq!(array.as_array(), Some(&vec.clone()));
        assert_eq!(array.as_array_mut(), Some(&mut vec.clone()));

        let mut map = BTreeMap::new();
        map.insert("key1".to_owned(), Value::String("value1".to_owned()));
        let mut dict = Value::Dictionary(map.clone());
        assert_eq!(dict.as_dictionary(), Some(&map.clone()));
        assert_eq!(dict.as_dictionary_mut(), Some(&mut map.clone()));

        assert_eq!(Value::Boolean(true).as_boolean(), Some(true));

        let slice: &[u8] = &[1, 2, 3];
        assert_eq!(Value::Data(slice.to_vec()).as_data(), Some(slice));
        assert_eq!(
            Value::Data(slice.to_vec()).into_data(),
            Some(slice.to_vec())
        );

        let date: Date = SystemTime::now().into();
        assert_eq!(Value::Date(date.clone()).as_date(), Some(&date));

        assert_eq!(Value::Real(0.0).as_real(), Some(0.0));
        assert_eq!(Value::Integer(1).as_integer(), Some(1));
        assert_eq!(Value::String("2".to_owned()).as_string(), Some("2"));
        assert_eq!(
            Value::String("t".to_owned()).into_string(),
            Some("t".to_owned())
        );
    }

    #[test]
    fn builder() {
        // Input
        let events = vec![
            StartDictionary(None),
            StringValue("Author".to_owned()),
            StringValue("William Shakespeare".to_owned()),
            StringValue("Lines".to_owned()),
            StartArray(None),
            StringValue("It is a tale told by an idiot,".to_owned()),
            StringValue("Full of sound and fury, signifying nothing.".to_owned()),
            EndArray,
            StringValue("Birthdate".to_owned()),
            IntegerValue(1564),
            StringValue("Height".to_owned()),
            RealValue(1.60),
            EndDictionary,
        ];

        let builder = Builder::new(events.into_iter().map(|e| Ok(e)));
        let plist = builder.build();

        // Expected output
        let mut lines = Vec::new();
        lines.push(Value::String("It is a tale told by an idiot,".to_owned()));
        lines.push(Value::String(
            "Full of sound and fury, signifying nothing.".to_owned(),
        ));

        let mut dict = BTreeMap::new();
        dict.insert(
            "Author".to_owned(),
            Value::String("William Shakespeare".to_owned()),
        );
        dict.insert("Lines".to_owned(), Value::Array(lines));
        dict.insert("Birthdate".to_owned(), Value::Integer(1564));
        dict.insert("Height".to_owned(), Value::Real(1.60));

        assert_eq!(plist.unwrap(), Value::Dictionary(dict));
    }
}
