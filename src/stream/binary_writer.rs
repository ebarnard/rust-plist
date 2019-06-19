use std::io::Write;

use std::collections::HashMap;
use stream::{Event, Writer};
use {Date, Error, Integer};
use {Serializer, Value};

/// Adapted from https://hg.python.org/cpython/file/3.4/Lib/plistlib.py
pub struct BinaryWriter<W: Write> {
    writer: W,
    value: Value,
    object_list: Vec<Value>,
    object_ref_table: HashMap<Value, u64>,
    last_ref_num: u64,
}

impl<W: Write> BinaryWriter<W> {
    pub fn new(writer: W, value: Value) -> Result<BinaryWriter<W>, Error> {
        match value {
            Value::Array(_) | Value::Dictionary(_) => {
                let object_list = Vec::new();
                let object_ref_table = HashMap::new();
                let last_ref_num = 0u64;
                Ok(BinaryWriter {
                    writer,
                    value,
                    object_list,
                    object_ref_table,
                    last_ref_num,
                })
            }
            _ => Err(Error::Serde(
                "root object needs to be an Array or Dictionary".into(),
            )),
        }
    }

    pub fn write(&self) -> Result<(), Error> {
        Err(Error::Serde("BinaryWriter unimplemented".into()))
    }
}
