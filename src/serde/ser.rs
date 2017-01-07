// Tests for the serializer and deserializer are located in tests/serde_/mod.rs.
// They can be run with `cargo test --features serde_tests`.

use serde_base::ser::{Error as SerdeError, Serialize, Serializer as SerdeSerializer};

use {Error, EventWriter, PlistEvent};

impl SerdeError for Error {
    fn custom<T: Into<String>>(msg: T) -> Self {
        Error::Serde(msg.into())
    }

    fn invalid_value(_: &str) -> Self {
        Error::InvalidData
    }
}

pub struct Serializer<W: EventWriter> {
    writer: W,
}

impl<W: EventWriter> Serializer<W> {
    pub fn new(writer: W) -> Serializer<W> {
        Serializer { writer: writer }
    }

    #[inline]
    fn emit(&mut self, event: PlistEvent) -> Result<(), <Self as SerdeSerializer>::Error> {
        Ok(self.writer.write(&event)?)
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    // Emit {key: value}
    fn single_key_dict(&mut self, key: String) -> Result<(), <Self as SerdeSerializer>::Error> {
        try!(self.emit(PlistEvent::StartDictionary(Some(1))));
        try!(self.emit(PlistEvent::StringValue(key)));
        Ok(())
    }

    fn single_key_dict_end(&mut self) -> Result<(), <Self as SerdeSerializer>::Error> {
        try!(self.emit(PlistEvent::EndDictionary));
        Ok(())
    }
}

impl<W: EventWriter> SerdeSerializer for Serializer<W> {
    type Error = Error;
    type SeqState = ();
    type TupleState = ();
    type TupleStructState = ();
    type TupleVariantState = ();
    type MapState = ();
    type StructState = ();
    type StructVariantState = ();

    fn serialize_bool(&mut self, v: bool) -> Result<(), Self::Error> {
        self.emit(PlistEvent::BooleanValue(v))
    }

    fn serialize_isize(&mut self, v: isize) -> Result<(), Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i8(&mut self, v: i8) -> Result<(), Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(&mut self, v: i16) -> Result<(), Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(&mut self, v: i32) -> Result<(), Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(&mut self, v: i64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::IntegerValue(v))
    }

    fn serialize_usize(&mut self, v: usize) -> Result<(), Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u8(&mut self, v: u8) -> Result<(), Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(&mut self, v: u16) -> Result<(), Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(&mut self, v: u32) -> Result<(), Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u64(&mut self, v: u64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::IntegerValue(v as i64))
    }

    fn serialize_f32(&mut self, v: f32) -> Result<(), Self::Error> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(&mut self, v: f64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::RealValue(v))
    }

    fn serialize_char(&mut self, v: char) -> Result<(), Self::Error> {
        let sstr = v.to_string();
        self.serialize_str(&sstr)
    }

    fn serialize_str(&mut self, value: &str) -> Result<(), Self::Error> {
        self.emit(PlistEvent::StringValue(value.to_owned()))
    }

    fn serialize_bytes(&mut self, value: &[u8]) -> Result<(), Self::Error> {
        self.emit(PlistEvent::DataValue(value.to_owned()))
    }

    fn serialize_unit(&mut self) -> Result<(), Self::Error> {
        // Emit empty string
        self.emit(PlistEvent::StringValue(String::new()))
    }

    fn serialize_unit_struct(&mut self, _name: &'static str) -> Result<(), Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(&mut self,
                              _name: &'static str,
                              _variant_index: usize,
                              variant: &'static str)
                              -> Result<(), Self::Error> {
        self.single_key_dict(variant.to_owned())?;
        self.serialize_unit()?;
        self.single_key_dict_end()?;
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize>(&mut self,
                                              _name: &'static str,
                                              value: T)
                                              -> Result<(), Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize>(&mut self,
                                               _name: &'static str,
                                               _variant_index: usize,
                                               variant: &'static str,
                                               value: T)
                                               -> Result<(), Self::Error> {
        self.single_key_dict(variant.to_owned())?;
        value.serialize(self)?;
        self.single_key_dict_end()
    }

    fn serialize_none(&mut self) -> Result<(), Self::Error> {
        self.single_key_dict("None".to_owned())?;
        self.serialize_unit()?;
        self.single_key_dict_end()
    }

    fn serialize_some<T: Serialize>(&mut self, value: T) -> Result<(), Self::Error> {
        self.single_key_dict("Some".to_owned())?;
        value.serialize(self)?;
        self.single_key_dict_end()
    }

    fn serialize_seq(&mut self, len: Option<usize>) -> Result<Self::SeqState, Self::Error> {
        let len = len.map(|len| len as u64);
        self.emit(PlistEvent::StartArray(len))
    }

    fn serialize_seq_elt<T: Serialize>(&mut self,
                                       _state: &mut Self::SeqState,
                                       value: T)
                                       -> Result<(), Self::Error> {
        value.serialize(self)
    }

    fn serialize_seq_end(&mut self, _state: Self::SeqState) -> Result<(), Self::Error> {
        self.emit(PlistEvent::EndArray)
    }

    fn serialize_seq_fixed_size(&mut self, size: usize) -> Result<Self::SeqState, Self::Error> {
        self.serialize_seq(Some(size))
    }

    fn serialize_tuple(&mut self, len: usize) -> Result<Self::TupleState, Self::Error> {
        self.serialize_seq_fixed_size(len)
    }

    fn serialize_tuple_elt<T: Serialize>(&mut self,
                                         state: &mut Self::TupleState,
                                         value: T)
                                         -> Result<(), Self::Error> {
        self.serialize_seq_elt(state, value)
    }

    fn serialize_tuple_end(&mut self, state: Self::TupleState) -> Result<(), Self::Error> {
        self.serialize_seq_end(state)
    }

    fn serialize_tuple_struct(&mut self,
                              _name: &'static str,
                              len: usize)
                              -> Result<Self::TupleStructState, Self::Error> {
        self.serialize_seq_fixed_size(len)
    }

    fn serialize_tuple_struct_elt<T: Serialize>(&mut self,
                                                state: &mut Self::TupleStructState,
                                                value: T)
                                                -> Result<(), Self::Error> {
        self.serialize_seq_elt(state, value)
    }

    fn serialize_tuple_struct_end(&mut self,
                                  state: Self::TupleStructState)
                                  -> Result<(), Self::Error> {
        self.serialize_seq_end(state)
    }

    fn serialize_tuple_variant(&mut self,
                               _name: &'static str,
                               _variant_index: usize,
                               variant: &'static str,
                               len: usize)
                               -> Result<Self::TupleVariantState, Self::Error> {

        self.single_key_dict(variant.to_owned())?;
        self.serialize_seq_fixed_size(len)
    }

    fn serialize_tuple_variant_elt<T: Serialize>(&mut self,
                                                 state: &mut Self::TupleVariantState,
                                                 value: T)
                                                 -> Result<(), Self::Error> {
        self.serialize_seq_elt(state, value)
    }
    fn serialize_tuple_variant_end(&mut self,
                                   state: Self::TupleVariantState)
                                   -> Result<(), Self::Error> {
        self.serialize_seq_end(state)?;
        self.single_key_dict_end()
    }


    fn serialize_map(&mut self, len: Option<usize>) -> Result<Self::MapState, Self::Error> {
        let len = len.map(|len| len as u64);
        self.emit(PlistEvent::StartDictionary(len))
    }

    fn serialize_map_key<T>(&mut self,
                            _state: &mut Self::MapState,
                            key: T)
                            -> Result<(), Self::Error>
        where T: Serialize
    {
        key.serialize(self)
    }

    fn serialize_map_value<T>(&mut self,
                              _state: &mut Self::MapState,
                              value: T)
                              -> Result<(), Self::Error>
        where T: Serialize
    {
        value.serialize(self)
    }

    fn serialize_map_end(&mut self, _state: Self::MapState) -> Result<(), Self::Error> {
        self.emit(PlistEvent::EndDictionary)
    }

    fn serialize_struct(&mut self,
                        _name: &'static str,
                        len: usize)
                        -> Result<Self::StructState, Self::Error> {
        self.serialize_map(Some(len))?;
        Ok(())
    }

    fn serialize_struct_elt<V: Serialize>(&mut self,
                                          state: &mut Self::StructState,
                                          key: &'static str,
                                          value: V)
                                          -> Result<(), Self::Error> {
        self.serialize_map_key(state, key)?;
        self.serialize_map_value(state, value)?;
        Ok(())
    }

    fn serialize_struct_end(&mut self, state: Self::StructState) -> Result<(), Self::Error> {
        self.serialize_map_end(state)
    }

    fn serialize_struct_variant(&mut self,
                                name: &'static str,
                                _variant_index: usize,
                                variant: &'static str,
                                len: usize)
                                -> Result<Self::StructVariantState, Self::Error> {
        self.single_key_dict(variant.to_owned())?;
        self.serialize_struct(name, len)?;
        Ok(())
    }

    fn serialize_struct_variant_elt<V: Serialize>(&mut self,
                                                  state: &mut Self::StructVariantState,
                                                  key: &'static str,
                                                  value: V)
                                                  -> Result<(), Self::Error> {
        self.serialize_struct_elt(state, key, value)
    }

    fn serialize_struct_variant_end(&mut self,
                                    state: Self::StructVariantState)
                                    -> Result<(), Self::Error> {
        self.serialize_struct_end(state)?;
        self.single_key_dict_end()?;
        Ok(())
    }
}
