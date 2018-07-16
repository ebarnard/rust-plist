use std::collections::BTreeMap;

use events::Event;
use {u64_option_to_usize, Error, Value};

pub struct Builder<T> {
    stream: T,
    token: Option<Event>,
}

impl<T: Iterator<Item = Result<Event, Error>>> Builder<T> {
    pub fn new(stream: T) -> Builder<T> {
        Builder {
            stream,
            token: None,
        }
    }

    pub fn build(mut self) -> Result<Value, Error> {
        self.bump()?;

        let plist = self.build_value()?;
        self.bump()?;
        match self.token {
            None => (),
            // The stream should have finished
            _ => return Err(Error::InvalidData),
        };
        Ok(plist)
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

    use super::*;
    use events::Event::*;
    use Value;

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
