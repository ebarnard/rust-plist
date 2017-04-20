// Tests for the serializer and deserializer are located in tests/serde_/mod.rs.
// They can be run with `cargo test --features serde_tests`.

use serde_base::de;
use std::iter::Peekable;
use std::fmt::Display;

use {Error, PlistEvent, u64_option_to_usize};

macro_rules! expect {
    ($next:expr, $pat:pat) => {
        match $next {
            Some(Ok(v@$pat)) => v,
            None => return Err(Error::UnexpectedEof),
            _ => return Err(event_mismatch_error())
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

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Serde(msg.to_string())
    }
}

pub struct Deserializer<I>
    where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    events: Peekable<<I as IntoIterator>::IntoIter>,
}

impl<I> Deserializer<I>
    where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    pub fn new(iter: I) -> Deserializer<I> {
        Deserializer { events: iter.into_iter().peekable() }
    }
}

impl<'a, I> de::Deserializer for &'a mut Deserializer<I>
    where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn deserialize<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor
    {
        match try_next!(self.events.next()) {
            PlistEvent::StartArray(len) => {
                let len = try!(u64_option_to_usize(len));
                let ret = visitor.visit_seq(MapAndSeqVisitor::new(self, len))?;
                expect!(self.events.next(), PlistEvent::EndArray);
                Ok(ret)
            }
            PlistEvent::EndArray => return Err(event_mismatch_error()),

            PlistEvent::StartDictionary(len) => {
                let len = try!(u64_option_to_usize(len));
                let ret = visitor.visit_map(MapAndSeqVisitor::new(self, len))?;
                expect!(self.events.next(), PlistEvent::EndDictionary);
                Ok(ret)
            }
            PlistEvent::EndDictionary => return Err(event_mismatch_error()),

            PlistEvent::BooleanValue(v) => visitor.visit_bool(v),
            PlistEvent::DataValue(v) => visitor.visit_byte_buf(v),
            PlistEvent::DateValue(v) => visitor.visit_string(v.to_string()),
            PlistEvent::IntegerValue(v) if v.is_positive() => visitor.visit_u64(v as u64),
            PlistEvent::IntegerValue(v) => visitor.visit_i64(v as i64),
            PlistEvent::RealValue(v) => visitor.visit_f64(v),
            PlistEvent::StringValue(v) => visitor.visit_string(v),
        }
    }

    forward_to_deserialize! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string
        seq seq_fixed_size bytes byte_buf map unit_struct
        tuple_struct struct struct_field tuple ignored_any
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor
    {
        expect!(self.events.next(), PlistEvent::StringValue(_));
        visitor.visit_unit()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor
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
            PlistEvent::StringValue(ref s) if &s[..] == "Some" => {
                try!(visitor.visit_some(&mut *self))
            }
            _ => return Err(event_mismatch_error()),
        };

        expect!(self.events.next(), PlistEvent::EndDictionary);

        Ok(ret)
    }

    fn deserialize_newtype_struct<V>(self,
                                     _name: &'static str,
                                     visitor: V)
                                     -> Result<V::Value, Self::Error>
        where V: de::Visitor
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(self,
                           _enum: &'static str,
                           _variants: &'static [&'static str],
                           visitor: V)
                           -> Result<V::Value, Self::Error>
        where V: de::Visitor
    {
        expect!(self.events.next(), PlistEvent::StartDictionary(_));
        let ret = try!(visitor.visit_enum(&mut *self));
        expect!(self.events.next(), PlistEvent::EndDictionary);
        Ok(ret)
    }
}

impl<'a, I> de::EnumVisitor for &'a mut Deserializer<I>
    where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;
    type Variant = Self;

    fn visit_variant_seed<V>(self, seed: V) -> Result<(V::Value, Self), Self::Error>
        where V: de::DeserializeSeed
    {
        Ok((seed.deserialize(&mut *self)?, self))
    }
}

impl<'a, I> de::VariantVisitor for &'a mut Deserializer<I>
    where I: IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn visit_unit(self) -> Result<(), Self::Error> {
        <() as de::Deserialize>::deserialize(self)
    }

    fn visit_newtype_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
        where T: de::DeserializeSeed
    {
        seed.deserialize(self)
    }

    fn visit_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where V: de::Visitor
    {
        <Self as de::Deserializer>::deserialize_tuple(self, len, visitor)
    }

    fn visit_struct<V>(self,
                       fields: &'static [&'static str],
                       visitor: V)
                       -> Result<V::Value, Self::Error>
        where V: de::Visitor
    {
        let name = "";
        <Self as de::Deserializer>::deserialize_struct(self, name, fields, visitor)
    }
}

struct MapAndSeqVisitor<'a, I>
    where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    de: &'a mut Deserializer<I>,
    remaining: Option<usize>,
}

impl<'a, I> MapAndSeqVisitor<'a, I>
    where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    fn new(de: &'a mut Deserializer<I>, len: Option<usize>) -> MapAndSeqVisitor<'a, I> {
        MapAndSeqVisitor {
            de: de,
            remaining: len,
        }
    }
}

impl<'a, I> de::SeqVisitor for MapAndSeqVisitor<'a, I>
    where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn visit_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
        where T: de::DeserializeSeed
    {
        match self.de.events.peek() {
            Some(&Ok(PlistEvent::EndArray)) => Ok(None),
            _ => {
                let ret = seed.deserialize(&mut *self.de).map(|k| Some(k));
                self.remaining = self.remaining.map(|r| r.saturating_sub(1));
                ret
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        <Self as de::MapVisitor>::size_hint(self)
    }
}

impl<'a, I> de::MapVisitor for MapAndSeqVisitor<'a, I>
    where I: 'a + IntoIterator<Item = Result<PlistEvent, Error>>
{
    type Error = Error;

    fn visit_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
        where K: de::DeserializeSeed
    {
        match self.de.events.peek() {
            Some(&Ok(PlistEvent::EndDictionary)) => return Ok(None),
            _ => {
                let ret = seed.deserialize(&mut *self.de).map(|k| Some(k));
                self.remaining = self.remaining.map(|r| r.saturating_sub(1));
                ret
            }
        }
    }

    fn visit_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
        where V: de::DeserializeSeed
    {
        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.remaining {
            Some(len) => (len, Some(len)),
            None => (0, None),
        }
    }
}
