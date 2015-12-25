use byteorder::{BigEndian, ReadBytesExt};
use byteorder::Error as ByteorderError;
use chrono::{TimeZone, UTC};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::string::FromUtf16Error;

use {Error, Result, PlistEvent, u64_to_usize};

impl From<ByteorderError> for Error {
    fn from(err: ByteorderError) -> Error {
        match err {
            ByteorderError::UnexpectedEOF => Error::UnexpectedEof,
            ByteorderError::Io(err) => Error::Io(err),
        }
    }
}

impl From<FromUtf16Error> for Error {
    fn from(_: FromUtf16Error) -> Error {
        Error::InvalidData
    }
}

struct StackItem {
    object_refs: Vec<u64>,
    ty: StackType,
}

enum StackType {
    Array,
    Dict,
    Root,
}

/// https://opensource.apple.com/source/CF/CF-550/CFBinaryPList.c
/// https://hg.python.org/cpython/file/3.4/Lib/plistlib.py
pub struct EventReader<R> {
    stack: Vec<StackItem>,
    object_offsets: Vec<u64>,
    reader: R,
    ref_size: u8,
    finished: bool,
}

impl<R: Read + Seek> EventReader<R> {
    pub fn new(reader: R) -> EventReader<R> {
        EventReader {
            stack: Vec::new(),
            object_offsets: Vec::new(),
            reader: reader,
            ref_size: 0,
            finished: false,
        }
    }

    fn read_trailer(&mut self) -> Result<()> {
        try!(self.reader.seek(SeekFrom::Start(0)));
        let mut magic = [0; 8];
        try!(self.reader.read(&mut magic));
        assert_eq!(&magic, b"bplist00");


        // Trailer starts with 6 bytes of padding
        try!(self.reader.seek(SeekFrom::End(-32 + 6)));
        let offset_size = try!(self.reader.read_u8());
        self.ref_size = try!(self.reader.read_u8());
        let num_objects = try!(self.reader.read_u64::<BigEndian>());
        let top_object = try!(self.reader.read_u64::<BigEndian>());
        let offset_table_offset = try!(self.reader.read_u64::<BigEndian>());

        // Read offset table
        try!(self.reader.seek(SeekFrom::Start(offset_table_offset)));
        self.object_offsets = try!(self.read_ints(num_objects, offset_size));

        // Seek to top object
        self.stack.push(StackItem {
            object_refs: vec![top_object],
            ty: StackType::Root,
        });

        Ok(())
    }

    fn read_ints(&mut self, len: u64, size: u8) -> Result<Vec<u64>> {
        let len = try!(u64_to_usize(len));
        let mut ints = Vec::with_capacity(len);
        // TODO: Is the match hoisted out of the loop?
        for _ in 0..len {
            match size {
                1 => ints.push(try!(self.reader.read_u8()) as u64),
                2 => ints.push(try!(self.reader.read_u16::<BigEndian>()) as u64),
                4 => ints.push(try!(self.reader.read_u32::<BigEndian>()) as u64),
                8 => ints.push(try!(self.reader.read_u64::<BigEndian>()) as u64),
                _ => return Err(Error::InvalidData),
            }
        }
        Ok(ints)
    }

    fn read_refs(&mut self, len: u64) -> Result<Vec<u64>> {
        let ref_size = self.ref_size;
        self.read_ints(len, ref_size)
    }

    fn read_object_len(&mut self, len: u8) -> Result<u64> {
        if (len & 0xf) == 0xf {
            let len_power_of_two = try!(self.reader.read_u8()) & 0x3;
            Ok(match len_power_of_two {
                0 => try!(self.reader.read_u8()) as u64,
                1 => try!(self.reader.read_u16::<BigEndian>()) as u64,
                2 => try!(self.reader.read_u32::<BigEndian>()) as u64,
                3 => try!(self.reader.read_u64::<BigEndian>()),
                _ => return Err(Error::InvalidData),
            })
        } else {
            Ok(len as u64)
        }
    }

    fn read_data(&mut self, len: u64) -> Result<Vec<u8>> {
        let len = try!(u64_to_usize(len));
        let mut data = vec![0; len];
        let mut total_read = 0;

        while total_read < len {
            let read = try!(self.reader.read(&mut data[total_read..]));
            if read == 0 {
                return Err(Error::UnexpectedEof);
            }
            total_read += read;
        }

        Ok(data)
    }

    fn seek_to_object(&mut self, object_ref: u64) -> Result<u64> {
        let object_ref = try!(u64_to_usize(object_ref));
        let offset = *&self.object_offsets[object_ref];
        let pos = try!(self.reader.seek(SeekFrom::Start(offset)));
        Ok(pos)
    }

    fn read_next(&mut self) -> Result<Option<PlistEvent>> {
        if self.ref_size == 0 {
            // Initialise here rather than in new
            try!(self.read_trailer());
            return Ok(Some(PlistEvent::StartPlist));
        }

        let object_ref = match self.stack.last_mut() {
            Some(stack_item) => stack_item.object_refs.pop(),
            // Reached the end of the plist
            None => return Ok(None),
        };

        match object_ref {
            Some(object_ref) => {
                try!(self.seek_to_object(object_ref));
            }
            None => {
                // We're at the end of an array or dict. Pop the top stack item and return
                let item = self.stack.pop().unwrap();
                match item.ty {
                    StackType::Array => return Ok(Some(PlistEvent::EndArray)),
                    StackType::Dict => return Ok(Some(PlistEvent::EndDictionary)),
                    StackType::Root => return Ok(Some(PlistEvent::EndPlist)),
                }
            }
        }

        let token = try!(self.reader.read_u8());
        let ty = (token & 0xf0) >> 4;
        let size = token & 0x0f;

        let result = match (ty, size) {
            (0x0, 0x00) => return Err(Error::InvalidData), // null
            (0x0, 0x08) => Some(PlistEvent::BooleanValue(false)),
            (0x0, 0x09) => Some(PlistEvent::BooleanValue(true)),
            (0x0, 0x0f) => return Err(Error::InvalidData), // fill
            (0x1, 0) => Some(PlistEvent::IntegerValue(try!(self.reader.read_u8()) as i64)),
            (0x1, 1) => {
                Some(PlistEvent::IntegerValue(try!(self.reader.read_u16::<BigEndian>()) as i64))
            }
            (0x1, 2) => {
                Some(PlistEvent::IntegerValue(try!(self.reader.read_u32::<BigEndian>()) as i64))
            }
            (0x1, 3) => Some(PlistEvent::IntegerValue(try!(self.reader.read_i64::<BigEndian>()))),
            (0x1, 4) => return Err(Error::InvalidData), // 128 bit int
            (0x1, _) => return Err(Error::InvalidData), // variable length int
            (0x2, 2) => {
                Some(PlistEvent::RealValue(try!(self.reader.read_f32::<BigEndian>()) as f64))
            }
            (0x2, 3) => Some(PlistEvent::RealValue(try!(self.reader.read_f64::<BigEndian>()))),
            (0x2, _) => return Err(Error::InvalidData), // odd length float
            (0x3, 3) => {
                // Date
                // Seconds since 1/1/2001 00:00:00
                let timestamp = try!(self.reader.read_f64::<BigEndian>());

                let secs = timestamp.floor();
                let subsecs = timestamp - secs;

                let int_secs = (secs as i64) + (31 * 365 + 8) * 86400;
                let int_nanos = (subsecs * 1_000_000_000f64) as u32;

                Some(PlistEvent::DateValue(UTC.timestamp(int_secs, int_nanos)))
            }
            (0x4, n) => {
                // Data
                let len = try!(self.read_object_len(n));
                Some(PlistEvent::DataValue(try!(self.read_data(len))))
            }
            (0x5, n) => {
                // ASCII string
                let len = try!(self.read_object_len(n));
                let raw = try!(self.read_data(len));
                let string = String::from_utf8(raw).unwrap();
                Some(PlistEvent::StringValue(string))
            }
            (0x6, n) => {
                // UTF-16 string
                // n is the length of code units (16 bits), not bytes.
                let len = try!(self.read_object_len(n * 2));
                let raw = try!(self.read_data(len));
                let mut cursor = Cursor::new(raw);

                let len_div_2 = try!(u64_to_usize(len / 2));
                let mut raw_utf16 = Vec::with_capacity(len_div_2);
                while cursor.position() < len {
                    raw_utf16.push(try!(cursor.read_u16::<BigEndian>()))
                }

                let string = try!(String::from_utf16(&raw_utf16));
                Some(PlistEvent::StringValue(string))
            }
            (0xa, n) => {
                // Array
                let len = try!(self.read_object_len(n));
                let mut object_refs = try!(self.read_refs(len));
                // Reverse so we can pop off the end of the stack in order
                object_refs.reverse();

                self.stack.push(StackItem {
                    ty: StackType::Array,
                    object_refs: object_refs,
                });

                Some(PlistEvent::StartArray(Some(len)))
            }
            (0xd, n) => {
                // Dict
                let len = try!(self.read_object_len(n));
                let key_refs = try!(self.read_refs(len));
                let value_refs = try!(self.read_refs(len));

                let len_mul_2 = try!(u64_to_usize(len * 2));
                let len = try!(u64_to_usize(len));

                let mut object_refs = Vec::with_capacity(len_mul_2);

                for i in 1..len + 1 {
                    // Reverse so we can pop off the end of the stack in order
                    object_refs.push(value_refs[len - i]);
                    object_refs.push(key_refs[len - i]);
                }

                self.stack.push(StackItem {
                    ty: StackType::Dict,
                    object_refs: object_refs,
                });

                Some(PlistEvent::StartDictionary(Some(len as u64)))
            }
            (_, _) => return Err(Error::InvalidData),
        };

        Ok(result)
    }
}

impl<R: Read + Seek> Iterator for EventReader<R> {
    type Item = Result<PlistEvent>;

    fn next(&mut self) -> Option<Result<PlistEvent>> {
        if self.finished {
            None
        } else {
            match self.read_next() {
                Ok(Some(event)) => Some(Ok(event)),
                Err(err) => {
                    self.finished = true;
                    Some(Err(err))
                }
                Ok(None) => {
                    self.finished = true;
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, UTC};
    use std::fs::File;
    use std::path::Path;

    use super::*;
    use PlistEvent;

    #[test]
    fn streaming_parser() {
        use PlistEvent::*;

        let reader = File::open(&Path::new("./tests/data/binary.plist")).unwrap();
        let streaming_parser = EventReader::new(reader);
        let events: Vec<PlistEvent> = streaming_parser.map(|e| e.unwrap()).collect();

        let comparison = &[StartPlist,
                           StartDictionary(Some(6)),
                           StringValue("Lines".to_owned()),
                           StartArray(Some(2)),
                           StringValue("It is a tale told by an idiot,".to_owned()),
                           StringValue("Full of sound and fury, signifying nothing.".to_owned()),
                           EndArray,
                           StringValue("Death".to_owned()),
                           IntegerValue(1564),
                           StringValue("Height".to_owned()),
                           RealValue(1.60),
                           StringValue("Birthdate".to_owned()),
                           DateValue(UTC.ymd(1981, 05, 16).and_hms(11, 32, 06)),
                           StringValue("Author".to_owned()),
                           StringValue("William Shakespeare".to_owned()),
                           StringValue("Data".to_owned()),
                           DataValue(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0]),
                           EndDictionary,
                           EndPlist];

        assert_eq!(events, comparison);
    }

    #[test]
    fn utf16_plist() {
        use PlistEvent::*;

        let reader = File::open(&Path::new("./tests/data/utf16_bplist.plist")).unwrap();
        let streaming_parser = EventReader::new(reader);
        let events: Vec<PlistEvent> = streaming_parser.map(|e| e.unwrap()).collect();
        assert_eq!(events[39], StringValue("\u{2605} or better".to_owned()));
    }
}
