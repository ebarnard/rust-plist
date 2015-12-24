use serde::ser::{MapVisitor, Serialize, Serializer as SerdeSerializer, SeqVisitor};

use {EventWriter, PlistEvent};

pub struct Serializer<W: EventWriter> {
    writer: W,
}

impl<W: EventWriter> Serializer<W> {
    pub fn new(writer: W) -> Serializer<W> {
        Serializer { writer: writer }
    }

    #[inline]
    fn emit(&mut self, event: PlistEvent) -> Result<(), ()> {
        self.writer.write(&event)
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    fn single_key_dict<F>(&mut self,
                          key: String,
                          value_fn: F)
                          -> Result<(), <Self as SerdeSerializer>::Error>
        where F: FnOnce(&mut Serializer<W>) -> Result<(), <Self as SerdeSerializer>::Error>
    {
        // Emit {key: value}
        try!(self.emit(PlistEvent::StartDictionary(Some(1))));
        try!(self.emit(PlistEvent::StringValue(key)));
        try!(value_fn(self));
        try!(self.emit(PlistEvent::EndDictionary));
        Ok(())
    }
}

impl<W: EventWriter> SerdeSerializer for Serializer<W> {
    type Error = ();

    fn visit_bool(&mut self, v: bool) -> Result<(), Self::Error> {
        self.emit(PlistEvent::BooleanValue(v))
    }

    fn visit_i64(&mut self, v: i64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::IntegerValue(v))
    }

    fn visit_u64(&mut self, v: u64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::IntegerValue(v as i64))
    }

    fn visit_f64(&mut self, v: f64) -> Result<(), Self::Error> {
        self.emit(PlistEvent::RealValue(v))
    }

    fn visit_str(&mut self, value: &str) -> Result<(), Self::Error> {
        self.emit(PlistEvent::StringValue(value.to_owned()))
    }

    fn visit_bytes(&mut self, value: &[u8]) -> Result<(), Self::Error> {
        self.emit(PlistEvent::DataValue(value.to_owned()))
    }

    fn visit_unit(&mut self) -> Result<(), Self::Error> {
        // Emit empty string
        self.emit(PlistEvent::StringValue(String::new()))
    }

    fn visit_none(&mut self) -> Result<(), Self::Error> {
        self.single_key_dict("None".to_owned(), |this| this.visit_unit())
    }

    fn visit_some<V>(&mut self, value: V) -> Result<(), Self::Error>
        where V: Serialize
    {
        self.single_key_dict("Some".to_owned(), |this| value.serialize(this))
    }

    fn visit_seq<V>(&mut self, mut visitor: V) -> Result<(), Self::Error>
        where V: SeqVisitor
    {
        let len = visitor.len().map(|len| len as u64);
        try!(self.emit(PlistEvent::StartArray(len)));
        loop {
            match try!(visitor.visit(self)) {
                Some(()) => (),
                None => break,
            }
        }
        try!(self.emit(PlistEvent::EndArray));
        Ok(())
    }

    fn visit_seq_elt<T>(&mut self, value: T) -> Result<(), Self::Error>
        where T: Serialize
    {
        value.serialize(self)
    }

    fn visit_map<V>(&mut self, mut visitor: V) -> Result<(), Self::Error>
        where V: MapVisitor
    {
        let len = visitor.len().map(|len| len as u64);
        try!(self.emit(PlistEvent::StartDictionary(len)));
        loop {
            match try!(visitor.visit(self)) {
                Some(()) => (),
                None => break,
            }
        }
        try!(self.emit(PlistEvent::EndDictionary));
        Ok(())
    }

    fn visit_map_elt<K, V>(&mut self, key: K, value: V) -> Result<(), Self::Error>
        where K: Serialize,
              V: Serialize
    {
        try!(key.serialize(self));
        try!(value.serialize(self));
        Ok(())
    }

    fn visit_unit_variant(&mut self,
                          _name: &'static str,
                          _variant_index: usize,
                          variant: &'static str)
                          -> Result<(), Self::Error> {
        self.single_key_dict(variant.to_owned(), |this| this.visit_unit())
    }

    fn visit_newtype_struct<T>(&mut self, _name: &'static str, value: T) -> Result<(), Self::Error>
        where T: Serialize
    {
        value.serialize(self)
    }

    fn visit_newtype_variant<T>(&mut self,
                                _name: &'static str,
                                _variant_index: usize,
                                variant: &'static str,
                                value: T)
                                -> Result<(), Self::Error>
        where T: Serialize
    {
        self.single_key_dict(variant.to_owned(), |this| value.serialize(this))
    }

    fn visit_tuple_variant<V>(&mut self,
                              _name: &'static str,
                              _variant_index: usize,
                              variant: &'static str,
                              visitor: V)
                              -> Result<(), Self::Error>
        where V: SeqVisitor
    {
        self.single_key_dict(variant.to_owned(),
                             |this| this.visit_tuple_struct(variant, visitor))
    }

    fn visit_struct_variant<V>(&mut self,
                               _name: &'static str,
                               _variant_index: usize,
                               variant: &'static str,
                               visitor: V)
                               -> Result<(), Self::Error>
        where V: MapVisitor
    {
        self.single_key_dict(variant.to_owned(),
                             |this| this.visit_struct(variant, visitor))
    }
}
