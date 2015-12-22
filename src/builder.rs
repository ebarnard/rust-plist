use std::collections::BTreeMap;

use {ReadError, ReadResult, Plist, PlistEvent};

pub type BuildResult<T> = Result<T, BuildError>;

#[derive(Debug)]
pub enum BuildError {
    InvalidEvent,
    UnsupportedDictionaryKey,
    ReadError(ReadError),
}

impl From<ReadError> for BuildError {
    fn from(err: ReadError) -> BuildError {
        BuildError::ReadError(err)
    }
}

pub struct Builder<T> {
    stream: T,
    token: Option<PlistEvent>,
}

impl<T: Iterator<Item = ReadResult<PlistEvent>>> Builder<T> {
    pub fn new(stream: T) -> Builder<T> {
        Builder {
            stream: stream,
            token: None,
        }
    }

    pub fn build(mut self) -> BuildResult<Plist> {
        try!(self.bump());
        if let Some(PlistEvent::StartPlist) = self.token {
            try!(self.bump());
        }

        let plist = try!(self.build_value());
        try!(self.bump());
        match self.token {
            None => (),
            Some(PlistEvent::EndPlist) => try!(self.bump()),
            // The stream should have finished
            _ => return Err(BuildError::InvalidEvent),
        };
        Ok(plist)
    }

    fn bump(&mut self) -> BuildResult<()> {
        self.token = match self.stream.next() {
            Some(Ok(token)) => Some(token),
            Some(Err(err)) => return Err(BuildError::ReadError(err)),
            None => None,
        };
        Ok(())
    }

    fn build_value(&mut self) -> BuildResult<Plist> {
        match self.token.take() {
            Some(PlistEvent::StartPlist) => Err(BuildError::InvalidEvent),
            Some(PlistEvent::EndPlist) => Err(BuildError::InvalidEvent),

            Some(PlistEvent::StartArray(len)) => Ok(Plist::Array(try!(self.build_array(len)))),
            Some(PlistEvent::StartDictionary(len)) => {
                Ok(Plist::Dictionary(try!(self.build_dict(len))))
            }

            Some(PlistEvent::BooleanValue(b)) => Ok(Plist::Boolean(b)),
            Some(PlistEvent::DataValue(d)) => Ok(Plist::Data(d)),
            Some(PlistEvent::DateValue(d)) => Ok(Plist::Date(d)),
            Some(PlistEvent::IntegerValue(i)) => Ok(Plist::Integer(i)),
            Some(PlistEvent::RealValue(f)) => Ok(Plist::Real(f)),
            Some(PlistEvent::StringValue(s)) => Ok(Plist::String(s)),

            Some(PlistEvent::EndArray) => Err(BuildError::InvalidEvent),
            Some(PlistEvent::EndDictionary) => Err(BuildError::InvalidEvent),

            // The stream should not have ended here
            None => Err(BuildError::InvalidEvent),
        }
    }

    fn build_array(&mut self, len: Option<u64>) -> Result<Vec<Plist>, BuildError> {
        let mut values = match len {
            Some(len) => Vec::with_capacity(len as usize),
            None => Vec::new(),
        };

        loop {
            try!(self.bump());
            if let Some(PlistEvent::EndArray) = self.token {
                self.token.take();
                return Ok(values);
            }
            values.push(try!(self.build_value()));
        }
    }

    fn build_dict(&mut self, _len: Option<u64>) -> Result<BTreeMap<String, Plist>, BuildError> {
        let mut values = BTreeMap::new();

        loop {
            try!(self.bump());
            match self.token.take() {
                Some(PlistEvent::EndDictionary) => return Ok(values),
                Some(PlistEvent::StringValue(s)) => {
                    try!(self.bump());
                    values.insert(s, try!(self.build_value()));
                }
                _ => {
                    // Only string keys are supported in plists
                    return Err(BuildError::UnsupportedDictionaryKey);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use Plist;

    #[test]
    fn builder() {
        use PlistEvent::*;

        // Input

        let events = vec![
			StartPlist,
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
			EndPlist,
		];

        let builder = Builder::new(events.into_iter().map(|e| Ok(e)));
        let plist = builder.build();

        // Expected output

        let mut lines = Vec::new();
        lines.push(Plist::String("It is a tale told by an idiot,".to_owned()));
        lines.push(Plist::String("Full of sound and fury, signifying nothing.".to_owned()));

        let mut dict = BTreeMap::new();
        dict.insert("Author".to_owned(),
                    Plist::String("William Shakespeare".to_owned()));
        dict.insert("Lines".to_owned(), Plist::Array(lines));
        dict.insert("Birthdate".to_owned(), Plist::Integer(1564));
        dict.insert("Height".to_owned(), Plist::Real(1.60));

        assert_eq!(plist.unwrap(), Plist::Dictionary(dict));
    }
}
