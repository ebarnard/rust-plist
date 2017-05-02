// Tests for the serializer and deserializer are located in tests/serde_/mod.rs.
// They can be run with `cargo test --features serde_tests`.

use serde_base::ser;
use std::fmt::Display;

use {Error, EventWriter, PlistEvent};

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Serde(msg.to_string())
    }
}

pub struct Serializer<W: EventWriter> {
    writer: W,
}

impl<W: EventWriter> Serializer<W> {
    pub fn new(writer: W) -> Serializer<W> {
        Serializer { writer: writer }
    }

    fn emit(&mut self, event: PlistEvent) -> Result<(), Error> {
        Ok(self.writer.write(&event)?)
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    // Emit {key: value}
    fn single_key_dict(&mut self, key: String) -> Result<(), Error> {
        try!(self.emit(PlistEvent::StartDictionary(Some(1))));
        try!(self.emit(PlistEvent::StringValue(key)));
        Ok(())
    }

    fn single_key_dict_end(&mut self) -> Result<(), Error> {
        try!(self.emit(PlistEvent::EndDictionary));
        Ok(())
    }
}

impl<'a, W: EventWriter> ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Compound<'a, W>;
    type SerializeTuple = Compound<'a, W>;
    type SerializeTupleStruct = Compound<'a, W>;
    type SerializeTupleVariant = Compound<'a, W>;
    type SerializeMap = Compound<'a, W>;
    type SerializeStruct = Compound<'a, W>;
    type SerializeStructVariant = Compound<'a, W>;

    fn serialize_bool(self, v: bool) -> Result<(), Self::Error> {
        self.emit(PlistEvent::BooleanValue(v))
    }

    fn serialize_i8(self, v: i8) -> Result<(), Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<(), Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<(), Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::IntegerValue(v))
    }

    fn serialize_u8(self, v: u8) -> Result<(), Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<(), Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<(), Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::IntegerValue(v as i64))
    }

    fn serialize_f32(self, v: f32) -> Result<(), Self::Error> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, v: f64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::RealValue(v))
    }

    fn serialize_char(self, v: char) -> Result<(), Self::Error> {
        self.emit(PlistEvent::StringValue(v.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<(), Self::Error> {
        self.emit(PlistEvent::StringValue(value.to_owned()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<(), Self::Error> {
        self.emit(PlistEvent::DataValue(value.to_owned()))
    }

    fn serialize_none(self) -> Result<(), Self::Error> {
        self.single_key_dict("None".to_owned())?;
        self.serialize_unit()?;
        self.single_key_dict_end()
    }

    fn serialize_some<T: ?Sized + ser::Serialize>(self, value: &T) -> Result<(), Self::Error> {
        self.single_key_dict("Some".to_owned())?;
        value.serialize(&mut *self)?;
        self.single_key_dict_end()
    }

    fn serialize_unit(self) -> Result<(), Self::Error> {
        // Emit empty string
        self.emit(PlistEvent::StringValue(String::new()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(self,
                              _name: &'static str,
                              _variant_index: u32,
                              variant: &'static str)
                              -> Result<(), Self::Error> {
        self.single_key_dict(variant.to_owned())?;
        self.serialize_unit()?;
        self.single_key_dict_end()?;
        Ok(())
    }

    fn serialize_newtype_struct<T: ?Sized + ser::Serialize>(self,
                                                            _name: &'static str,
                                                            value: &T)
                                                            -> Result<(), Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + ser::Serialize>(self,
                                                             _name: &'static str,
                                                             _variant_index: u32,
                                                             variant: &'static str,
                                                             value: &T)
                                                             -> Result<(), Self::Error> {
        self.single_key_dict(variant.to_owned())?;
        value.serialize(&mut *self)?;
        self.single_key_dict_end()
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let len = len.map(|len| len as u64);
        self.emit(PlistEvent::StartArray(len))?;
        Ok(Compound { ser: self })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self,
                              _name: &'static str,
                              len: usize)
                              -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_tuple(len)
    }

    fn serialize_tuple_variant(self,
                               _name: &'static str,
                               _variant_index: u32,
                               variant: &'static str,
                               len: usize)
                               -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.single_key_dict(variant.to_owned())?;
        self.serialize_tuple(len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        let len = len.map(|len| len as u64);
        self.emit(PlistEvent::StartDictionary(len))?;
        Ok(Compound { ser: self })
    }

    fn serialize_struct(self,
                        _name: &'static str,
                        len: usize)
                        -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(self,
                                name: &'static str,
                                _variant_index: u32,
                                variant: &'static str,
                                len: usize)
                                -> Result<Self::SerializeStructVariant, Self::Error> {
        self.single_key_dict(variant.to_owned())?;
        self.serialize_struct(name, len)
    }
}

#[doc(hidden)]
pub struct Compound<'a, W: 'a + EventWriter> {
    ser: &'a mut Serializer<W>,
}

impl<'a, W: EventWriter> ser::SerializeSeq for Compound<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + ser::Serialize>(&mut self,
                                                     value: &T)
                                                     -> Result<(), Self::Error> {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.ser.emit(PlistEvent::EndArray)
    }
}

impl<'a, W: EventWriter> ser::SerializeTuple for Compound<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + ser::Serialize>(&mut self,
                                                     value: &T)
                                                     -> Result<(), Self::Error> {
        <Self as ser::SerializeSeq>::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as ser::SerializeSeq>::end(self)
    }
}

impl<'a, W: EventWriter> ser::SerializeTupleStruct for Compound<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + ser::Serialize>(&mut self,
                                                   value: &T)
                                                   -> Result<(), Self::Error> {
        <Self as ser::SerializeSeq>::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as ser::SerializeSeq>::end(self)
    }
}

impl<'a, W: EventWriter> ser::SerializeTupleVariant for Compound<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + ser::Serialize>(&mut self,
                                                   value: &T)
                                                   -> Result<(), Self::Error> {
        <Self as ser::SerializeSeq>::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.ser.emit(PlistEvent::EndArray)?;
        self.ser.single_key_dict_end()
    }
}

impl<'a, W: EventWriter> ser::SerializeMap for Compound<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + ser::Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        key.serialize(&mut *self.ser)
    }

    fn serialize_value<T: ?Sized + ser::Serialize>(&mut self,
                                                   value: &T)
                                                   -> Result<(), Self::Error> {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.ser.emit(PlistEvent::EndDictionary)
    }
}

impl<'a, W: EventWriter> ser::SerializeStruct for Compound<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + ser::Serialize>(&mut self,
                                                   key: &'static str,
                                                   value: &T)
                                                   -> Result<(), Self::Error> {
        <Self as ser::SerializeMap>::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as ser::SerializeMap>::end(self)
    }
}

impl<'a, W: EventWriter> ser::SerializeStructVariant for Compound<'a, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + ser::Serialize>(&mut self,
                                                   key: &'static str,
                                                   value: &T)
                                                   -> Result<(), Self::Error> {
        <Self as ser::SerializeStruct>::serialize_field(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.ser.emit(PlistEvent::EndDictionary)?;
        self.ser.single_key_dict_end()
    }
}
