// Tests for the serializer and deserializer are located in tests/serde_/mod.rs.
// They can be run with `cargo test --features serde_tests`.

use serde::de::{Deserializer as SerdeDeserializer, Error as SerdeError, Visitor, SeqVisitor,
                MapVisitor, VariantVisitor, Deserialize, EnumVisitor};
use std::iter::Peekable;

use {Error, PlistEvent, u64_option_to_usize};

macro_rules! expect {
    ($next:expr, $pat:pat) => {
        match $next {
            Some(Ok(v@$pat)) => v,
            None => return Err(Error::UnexpectedEof),
            _ => return return Err(event_mismatch_error())
        }
    };
    ($next:expr, $pat:pat => $save:expr) => {
        match $next {
            Some(Ok($pat)) => $save,
            None => return Err(Error::UnexpectedEof),
            _ => return Err(event_mismatch_error())
        }
    };
}

macro_rules! try_next {
    ($next:expr) => {
        match $next {
            Some(Ok(v)) => v,
            Some(Err(_)) => return Err(event_mismatch_error()),
            None => return Err(Error::UnexpectedEof)
        }
    }
}

fn event_mismatch_error() -> Error {
    Error::InvalidData
}

impl SerdeError for Error {
    fn custom<T: Into<String>>(msg: T) -> Self {
        Error::Serde(msg.into())
    }

    fn end_of_stream() -> Self {
        Error::UnexpectedEof
    }
}

pub struct Deserializer<I>
    where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    events: Peekable<<I as IntoIterator>::IntoIter>,
}

impl<I> Deserializer<I> where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    pub fn new(iter: I) -> Deserializer<I> {
        Deserializer { events: iter.into_iter().peekable() }
    }
}

impl<I> SerdeDeserializer for Deserializer<I>
    where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn deserialize<V>(&mut self, mut visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor
    {
        match try_next!(self.events.next()) {
            PlistEvent::StartArray(len) => {
                let len = try!(u64_option_to_usize(len));
                visitor.visit_seq(MapSeq::new(self, len))
            }
            PlistEvent::EndArray => return Err(event_mismatch_error()),

            PlistEvent::StartDictionary(len) => {
                let len = try!(u64_option_to_usize(len));
                visitor.visit_map(MapSeq::new(self, len))
            }
            PlistEvent::EndDictionary => return Err(event_mismatch_error()),

            PlistEvent::BooleanValue(v) => visitor.visit_bool(v),
            PlistEvent::DataValue(v) => visitor.visit_byte_buf(v),
            PlistEvent::DateValue(v) => visitor.visit_string(v.to_rfc3339()),
            PlistEvent::IntegerValue(v) if v.is_positive() => visitor.visit_u64(v as u64),
            PlistEvent::IntegerValue(v) => visitor.visit_i64(v as i64),
            PlistEvent::RealValue(v) => visitor.visit_f64(v),
            PlistEvent::StringValue(v) => visitor.visit_string(v),
        }
    }

    fn deserialize_unit<V>(&mut self, mut visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor
    {
        expect!(self.events.next(), PlistEvent::StringValue(_));
        visitor.visit_unit()
    }

    fn deserialize_option<V>(&mut self, mut visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor
    {
        expect!(self.events.next(), PlistEvent::StartDictionary(_));

        let ret = match try_next!(self.events.next()) {
            PlistEvent::StringValue(ref s) if &s[..] == "None" => {
                let ret = match visitor.visit_none() {
                    Ok(ret) => ret,
                    Err(e) => return Err(e),
                };
                // For some reason the try! below doesn't work - probably a macro hygene issue
                // with Error and ::Error
                // let ret = try!(visitor.visit_none());
                expect!(self.events.next(), PlistEvent::StringValue(_));
                ret
            }
            PlistEvent::StringValue(ref s) if &s[..] == "Some" => try!(visitor.visit_some(self)),
            _ => return Err(event_mismatch_error()),
        };

        expect!(self.events.next(), PlistEvent::EndDictionary);

        Ok(ret)
    }

    fn deserialize_newtype_struct<V>(&mut self,
                               _name: &'static str,
                               mut visitor: V)
                               -> Result<V::Value, Self::Error>
        where V: Visitor
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(&mut self,
                     _enum: &'static str,
                     _variants: &'static [&'static str],
                     mut visitor: V)
                     -> Result<V::Value, Self::Error>
        where V: EnumVisitor
    {
        expect!(self.events.next(), PlistEvent::StartDictionary(_));
        let ret = try!(visitor.visit(&mut *self));
        expect!(self.events.next(), PlistEvent::EndDictionary);
        Ok(ret)
    }
}

impl<I> VariantVisitor for Deserializer<I> where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn visit_variant<V>(&mut self) -> Result<V, Self::Error>
        where V: Deserialize
    {
        <V as Deserialize>::deserialize(self)
    }

    fn visit_unit(&mut self) -> Result<(), Self::Error> {
        <() as Deserialize>::deserialize(self)
    }

    fn visit_newtype<T>(&mut self) -> Result<T, Self::Error>
        where T: Deserialize
    {
        <T as Deserialize>::deserialize(self)
    }

    fn visit_tuple<V>(&mut self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where V: Visitor
    {
        <Self as SerdeDeserializer>::deserialize_tuple(self, len, visitor)
    }

    fn visit_struct<V>(&mut self,
                       fields: &'static [&'static str],
                       visitor: V)
                       -> Result<V::Value, Self::Error>
        where V: Visitor
    {
        let name = "";
        <Self as SerdeDeserializer>::deserialize_struct(self, name, fields, visitor)
    }
}

struct MapSeq<'a, I>
    where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    de: &'a mut Deserializer<I>,
    remaining: Option<usize>,
    finished: bool,
}

impl<'a, I> MapSeq<'a, I> where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    fn new(de: &'a mut Deserializer<I>, len: Option<usize>) -> MapSeq<'a, I> {
        MapSeq {
            de: de,
            remaining: len,
            finished: false,
        }
    }
}

impl<'a, I> SeqVisitor for MapSeq<'a, I>
    where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn visit<T>(&mut self) -> Result<Option<T>, Self::Error>
        where T: Deserialize
    {
        match self.de.events.peek() {
            Some(&Ok(PlistEvent::EndArray)) => {
                self.de.events.next();
                self.finished = true;
                return Ok(None);
            }
            _ => <T as Deserialize>::deserialize(self.de).map(|k| Some(k)),
        }
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        if !self.finished {
            self.finished = true;
            expect!(self.de.events.next(), PlistEvent::EndArray);
        }
        Ok(())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        <Self as MapVisitor>::size_hint(self)
    }
}

impl<'a, I> MapVisitor for MapSeq<'a, I>
    where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn visit_key<K>(&mut self) -> Result<Option<K>, Self::Error>
        where K: Deserialize
    {
        match self.de.events.peek() {
            Some(&Ok(PlistEvent::EndDictionary)) => {
                self.de.events.next();
                self.finished = true;
                return Ok(None);
            }
            _ => <K as Deserialize>::deserialize(self.de).map(|k| Some(k)),
        }
    }

    fn visit_value<V>(&mut self) -> Result<V, Self::Error>
        where V: Deserialize
    {
        <V as Deserialize>::deserialize(self.de)
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        if !self.finished {
            self.finished = true;
            expect!(self.de.events.next(), PlistEvent::EndDictionary);
        }
        Ok(())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.remaining {
            Some(len) => (len, Some(len)),
            None => (0, None),
        }
    }
}
