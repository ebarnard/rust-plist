use std::io::Write;
use std::collections::HashMap;
use super::{Date, Error, Integer, Value};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum RefSize {
    U8,
    U16,
    U32,
    U64,
}

impl RefSize {
    fn into_offset_size(self) -> u8 {
        match self {
            RefSize::U8 => 1,
            RefSize::U16 => 2,
            RefSize::U32 => 4,
            RefSize::U64 => 8,
        }
    }
}

/// Adapted from https://hg.python.org/cpython/file/3.4/Lib/plistlib.py
pub struct BinaryWriter<W: Write> {
    writer: W,
    value: Value,
    object_list: Vec<Value>,
    object_ref_table: HashMap<Value, usize>,
    object_offsets: Vec<usize>,
    flattened: bool,
    written: usize,
    ref_size: RefSize,
}

impl<W: Write> BinaryWriter<W> {
    pub fn new(writer: W, value: Value) -> Result<BinaryWriter<W>, Error> {
        match value {
            Value::Array(_) | Value::Dictionary(_) => Ok(BinaryWriter {
                writer,
                value,
                object_list: Vec::new(),
                object_ref_table: HashMap::new(),
                object_offsets: Vec::new(),
                flattened: false,
                written: 0,
                ref_size: RefSize::U8,
            }),
            _ => Err(Error::Serde(
                "root object needs to be an Array or Dictionary".into(),
            )),
        }
    }

    pub fn write(&mut self) -> Result<usize, Error> {
        self.flatten();
        let num_objects = self.object_list.len();
        for _ in 0..num_objects {
            self.object_offsets.push(0);
        }
        self.ref_size = BinaryWriter::<W>::size_of_count(&num_objects);

        // write bplist header
        self.written += self.write_header()?;

        // write object list
        // TODO: get rid of this clone
        for o in self.object_list.clone() {
            self.written += self.write_object(&o)?;
        }

        // write offset table
        let top_object_ref_num = self.expect_ref_num(&self.value)?;
        let offset_table_offset = self.written;
        let offset_table_offset_size = BinaryWriter::<W>::size_of_count(&offset_table_offset);
        // TODO: get rid of this clone
        for offset in self.object_offsets.clone() {
            self.written += self.write_int_sized(offset_table_offset_size, offset)?;
        }

        // write trailer
        let sort_version = 0;

        self.written += self.writer.write(&[
            0, // first 4 bytes are unused
            0,
            0,
            0,
            sort_version, // then sort_version, whatever that is, which is apparently 0
            offset_table_offset_size.into_offset_size(), // followed by size of an offset table entry
            self.ref_size.clone().into_offset_size(),    // followed size of a reference entry
        ])?;
        // number of objects in object list, as u64be
        self.written += self.writer.write(&(num_objects as u64).to_be_bytes())?;
        // reference number of root object, as u64be
        self.written += self
            .writer
            .write(&(top_object_ref_num as u64).to_be_bytes())?;
        // start of offset table, relative to file.
        self.written += self
            .writer
            .write(&(offset_table_offset as u64).to_be_bytes())?;
        Ok(self.written)
    }

    fn write_header(&mut self) -> Result<usize, Error> {
        let count = self.writer.write(b"bplist00")?;
        Ok(count)
    }

    fn write_size(&mut self, token: u8, size: usize) -> Result<usize, Error> {
        let mut count = 0usize;
        if size < 15 {
            count += self.writer.write(&[token | (size as u8 & 0xF)])?;
        } else if size < (1 << 8) {
            count += self.writer.write(&[token | 0xF, 0x10, size as u8])?;
        } else if size < (1 << 16) {
            count += self.writer.write(&[token | 0xF, 0x11])?;
            count += self.writer.write(&(size as u16).to_be_bytes())?;
        } else if size < (1 << 32) {
            count += self.writer.write(&[token | 0xF, 0x12])?;
            count += self.writer.write(&(size as u32).to_be_bytes())?;
        } else {
            count += self.writer.write(&[token | 0xF, 0x13])?;
            count += self.writer.write(&(size as u64).to_be_bytes())?;
        }
        Ok(count)
    }

    fn write_int_sized(&mut self, ref_size: RefSize, ref_num: usize) -> Result<usize, Error> {
        let bytes = match ref_size {
            RefSize::U8 => [ref_num as u8].to_vec(),
            RefSize::U16 => (ref_num as u16).to_be_bytes().to_vec(),
            RefSize::U32 => (ref_num as u16).to_be_bytes().to_vec(),
            RefSize::U64 => (ref_num as u64).to_be_bytes().to_vec(),
        };
        self.writer.write(bytes.as_slice()).map_err(Into::into)
    }

    fn write_ref_num(&mut self, ref_num: usize) -> Result<usize, Error> {
        self.write_int_sized(self.ref_size, ref_num)
    }

    fn write_object(&mut self, v: &Value) -> Result<usize, Error> {
        let ref_num = self.expect_ref_num(v)?;
        self.object_offsets[ref_num] = self.written;
        let mut count = 0usize;
        match v {
            Value::Boolean(b) => {
                if *b {
                    count += self.writer.write(&[0x08])?;
                } else {
                    count += self.writer.write(&[0x09])?;
                }
            }
            Value::Integer(int) => {
                let i = int.clone().into_inner();
                if i < 0i128 {
                    count += self.writer.write(&[0x13])?;
                    count += self.writer.write(&(i as i64).to_be_bytes())?;
                } else if i < (1 << 8) {
                    count += self.writer.write(&[0x10, i as u8])?;
                } else if i < (1 << 16) {
                    count += self.writer.write(&[0x11])?;
                    count += self.writer.write(&(i as u16).to_be_bytes())?;
                } else if i < (1 << 32) {
                    count += self.writer.write(&[0x12])?;
                    count += self.writer.write(&(i as u32).to_be_bytes())?;
                } else if i < (1 << 63) {
                    count += self.writer.write(&[0x13])?;
                    count += self.writer.write(&(i as u64).to_be_bytes())?;
                } else if i < (1 << 64) {
                    count += self.writer.write(&[0x14])?;
                    count += self.writer.write(&i.to_be_bytes())?;
                } else {
                    return Err(Error::Serde(format!(
                        "integer {} overflows plist min/max",
                        i
                    )));
                }
            }
            Value::Real(r) => {
                count += self.writer.write(&[0x23])?;
                count += self.writer.write(&r.to_bits().to_be_bytes())?;
            }
            Value::Date(d) => {
                let secs = &d.to_seconds_since_plist_epoch();
                count += self.writer.write(&[0x33])?;
                count += self.writer.write(&secs.to_bits().to_be_bytes())?;
            }
            Value::Data(d) => {
                count += self.write_size(0x40, d.len())?;
                count += self.writer.write(d.as_slice())?;
            }
            Value::String(s) => {
                if s.is_ascii() {
                    let ascii = s.as_bytes();
                    count += self.write_size(0x50, ascii.len())?;
                    count += self.writer.write(ascii)?;
                } else {
                    let utf16: Vec<u16> = s.encode_utf16().collect();
                    count += self.write_size(0x60, utf16.len())?;
                    for c in utf16 {
                        count += self.writer.write(&c.to_be_bytes())?;
                    }
                }
            }
            Value::Array(a) => {
                count += self.write_size(0xA0, a.len())?;
                for elem in a {
                    let ref_num = self.expect_ref_num(elem)?;
                    count += self.write_ref_num(ref_num)?;
                }
            }
            Value::Dictionary(d) => {
                let (mut key_refs, mut val_refs) = (Vec::new(), Vec::new());
                for (k, v) in d.iter() {
                    key_refs.push(self.expect_ref_num(&Value::String(k.clone()))?);
                    val_refs.push(self.expect_ref_num(v)?);
                }
                count += self.write_size(0xD0, key_refs.len())?;
                for kr in key_refs {
                    count += self.write_ref_num(kr)?;
                }
                for vr in val_refs {
                    count += self.write_ref_num(vr)?;
                }
            }
            Value::__Nonexhaustive => unreachable!(),
        }

        Ok(count)
    }

    fn flatten(&mut self) {
        if !self.flattened {
            self.flatten_inner(&self.value.clone());
            self.flattened = true;
        }
    }

    fn flatten_inner(&mut self, v: &Value) {
        self.upsert_to_object_list(v);
        match v {
            Value::Dictionary(d) => {
                let mut keys = Vec::new();
                let mut values = Vec::new();
                for (k, v) in d {
                    keys.push(Value::String(k.clone()));
                    values.push(v);
                }

                keys.iter().for_each(|k| self.flatten_inner(k));
                values.iter().for_each(|v| self.flatten_inner(v));
            }
            Value::Array(a) => {
                for v in a {
                    self.flatten_inner(v)
                }
            }
            _ => (),
        }
    }

    fn get_ref_num(&self, v: &Value) -> Option<usize> {
        return self.object_ref_table.get(v).map(Clone::clone);
    }

    fn expect_ref_num(&self, v: &Value) -> Result<usize, Error> {
        match self.get_ref_num(v) {
            Some(ref_num) => Ok(ref_num),
            None => Err(Error::Serde(format!(
                "expecting {:?} to already exist in ref table",
                v
            ))),
        }
    }

    fn upsert_to_object_list(&mut self, v: &Value) -> usize {
        match self.get_ref_num(v) {
            Some(ref_num) => ref_num,
            None => {
                let ref_num = self.object_list.len();
                self.object_list.push(v.clone());
                self.object_ref_table.insert(v.clone(), ref_num);
                ref_num
            }
        }
    }

    fn size_of_count(count: &usize) -> RefSize {
        let ret = if count < &(1 << 8) {
            RefSize::U8
        } else if count < &(1 << 16) {
            RefSize::U16
        } else if count < &(1 << 32) {
            RefSize::U32
        } else {
            RefSize::U64
        };
        ret
    }
}

#[cfg(test)]
mod tests {
    use humantime::parse_rfc3339_weak;
    use std::fs::File;
    use std::path::Path;

    use super::*;
    use std::io::Cursor;
    use stream::BinaryReader;
    use stream::Event;
    use stream::Event::*;

    fn test_roundtrip(path: &Path) {
        let reader = File::open(path).unwrap();
        let streaming_parser = BinaryReader::new(reader);
        let value_to_encode = Value::from_events(streaming_parser).unwrap();

        let mut buf = Cursor::new(Vec::new());
        let value_encoded = value_to_encode.to_writer(&mut buf).unwrap();

        let buf_inner = buf.into_inner();

        let streaming_parser = BinaryReader::new(Cursor::new(buf_inner));
        let value_decoded_from_encode = Value::from_events(streaming_parser).unwrap();

        assert_eq!(value_to_encode, value_decoded_from_encode);
    }

    #[test]
    fn bplist_roundtrip() {
        test_roundtrip(&Path::new("./tests/data/binary.plist"))
    }

    #[test]
    fn utf16_roundtrip() {
        test_roundtrip(&Path::new("./tests/data/utf16_bplist.plist"))
    }
}
