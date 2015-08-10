use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::UTF_16BE;
use encoding::types::{DecoderTrap, Encoding};
use itertools::Interleave;
use std::io::{Read, Seek, SeekFrom};
use std::io::Result as IoResult;

use super::PlistEvent;

struct StackItem {
	object_refs: Vec<u64>,
	ty: StackType
}

enum StackType {
	Array,
	Dict,
	Root
}

/// https://opensource.apple.com/source/CF/CF-550/CFBinaryPList.c
/// https://hg.python.org/cpython/file/3.4/Lib/plistlib.py
pub struct StreamingParser<R> {
	stack: Vec<StackItem>,
	object_offsets: Vec<u64>,
	reader: R,
	ref_size: u8
}

impl<R: Read+Seek> StreamingParser<R> {
	pub fn open(reader: R) -> Result<StreamingParser<R>, ()> {
		let mut parser = StreamingParser {
			stack: Vec::new(),
			object_offsets: Vec::new(),
			reader: reader,
			ref_size: 0
		};
		
		match parser.read_trailer() {
			Ok(_) => (),
			Err(_) => return Err(())
		}
		
		Ok(parser)
	}

	fn read_trailer(&mut self) -> IoResult<()> {
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
			ty: StackType::Root
		});

		Ok(())
	}

	fn read_ints(&mut self, len: u64, size: u8) -> IoResult<Vec<u64>> {
		let mut ints = Vec::with_capacity(len as usize);
		// TODO: Is the match hoisted out of the loop?
		// Is this even right wrt 1,2,4,8 etc. Should it be 0,1,2...
		for _ in 0..len {
			match size {
				1 => ints.push(try!(self.reader.read_u8()) as u64),
				2 => ints.push(try!(self.reader.read_u16::<BigEndian>()) as u64),
				4 => ints.push(try!(self.reader.read_u32::<BigEndian>()) as u64),
				8 => ints.push(try!(self.reader.read_u64::<BigEndian>()) as u64),
				_ => panic!("wtf")
			}
		}
		Ok(ints)
	}

	fn read_refs(&mut self, len: u64) -> IoResult<Vec<u64>> {
		let ref_size = self.ref_size;
		self.read_ints(len, ref_size)
	}

	fn read_object_len(&mut self, len: u8) -> IoResult<u64> {
		if (len & 0xf) == 0xf {
			let len_power_of_two = try!(self.reader.read_u8()) & 0x3;
			Ok(match len_power_of_two {
				0 => try!(self.reader.read_u8()) as u64,
				1 => try!(self.reader.read_u16::<BigEndian>()) as u64,
				2 => try!(self.reader.read_u32::<BigEndian>()) as u64,
				3 => try!(self.reader.read_u64::<BigEndian>()),
				_ => panic!("wrong len {}", len_power_of_two)
			})
		} else {
			Ok(len as u64)
		}
	}

	fn read_data(&mut self, len: u64) -> IoResult<Vec<u8>> {
		let mut data = vec![0; len as usize];
		let read_len = try!(self.reader.read(&mut data));
		if (read_len as u64) != len { panic!("not enough read") }
		Ok(data)
	}

	fn seek_to_object(&mut self, object_ref: u64) -> IoResult<u64> {
		// TODO: Better ways to deal with this cast?
		// I geuss not store the table locally if it's huge
		let offset = *&self.object_offsets[object_ref as usize];
		self.reader.seek(SeekFrom::Start(offset))
	}

	fn read_next(&mut self) -> IoResult<Option<PlistEvent>> {
		let object_ref = match self.stack.last_mut() {
			Some(stack_item) => stack_item.object_refs.pop(),
			// Reached the end of the plist
			None => return Ok(None)
		};

		match object_ref {
			Some(object_ref) => {
				try!(self.seek_to_object(object_ref));
			},
			None => {
				// We're at the end of an array or dict. Pop the top stack item and return
				let item = self.stack.pop().unwrap();
				match item.ty {
					StackType::Array => return Ok(Some(PlistEvent::EndArray)),
					StackType::Dict => return Ok(Some(PlistEvent::EndDictionary)),
					StackType::Root => return Ok(None) // Reached the end of the plist
				}
			}
		}

		let token = try!(self.reader.read_u8());
		let ty = (token & 0xf0) >> 4;
		let size = token & 0x0f;

		let result = match (ty, size) {
			(0x0, 0x00) => panic!("null"),
			(0x0, 0x08) => Some(PlistEvent::BooleanValue(false)),
			(0x0, 0x09) => Some(PlistEvent::BooleanValue(true)),
			(0x0, 0x0f) => panic!("fill"),
			(0x1, 0) => Some(PlistEvent::IntegerValue(try!(self.reader.read_u8()) as i64)),
			(0x1, 1) => Some(PlistEvent::IntegerValue(try!(self.reader.read_u16::<BigEndian>()) as i64)),
			(0x1, 2) => Some(PlistEvent::IntegerValue(try!(self.reader.read_u32::<BigEndian>()) as i64)),
			(0x1, 3) => Some(PlistEvent::IntegerValue(try!(self.reader.read_i64::<BigEndian>()))),
			(0x1, 4) => panic!("128 bit int"),
			(0x1, _) => panic!("variable length int"),
			(0x2, 2) => Some(PlistEvent::RealValue(try!(self.reader.read_f32::<BigEndian>()) as f64)),
			(0x2, 3) => Some(PlistEvent::RealValue(try!(self.reader.read_f64::<BigEndian>()))),
			(0x2, _) => panic!("odd length float"),
			(0x3, 3) => panic!("date"),
			(0x4, n) => { // data
				let len = try!(self.read_object_len(n));
				Some(PlistEvent::DataValue(try!(self.read_data(len))))
			},
			(0x5, n) => { // ASCII string
				let len = try!(self.read_object_len(n));
				let raw = try!(self.read_data(len));
				let string = String::from_utf8(raw).unwrap();
				Some(PlistEvent::StringValue(string))
			},
			(0x6, n) => { // UTF-16 string
				let len = try!(self.read_object_len(n));
				let raw = try!(self.read_data(len));
				let string = UTF_16BE.decode(&raw, DecoderTrap::Strict).unwrap();
				Some(PlistEvent::StringValue(string))
			},
			(0xa, n) => { // Array
				let len = try!(self.read_object_len(n));
				let mut object_refs = try!(self.read_refs(len));
				// Reverse so we can pop off the end of the stack in order
				object_refs.reverse();

				self.stack.push(StackItem {
					ty: StackType::Array,
					object_refs: object_refs
				});

				Some(PlistEvent::StartArray)
			},
			(0xd, n) => { // Dict
				let len = try!(self.read_object_len(n));
				let key_refs = try!(self.read_refs(len));
				let value_refs = try!(self.read_refs(len));

				let mut object_refs: Vec<_> = Interleave::new(key_refs.into_iter(), value_refs.into_iter()).collect();
				// Reverse so we can pop off the end of the stack in order
				object_refs.reverse();

				self.stack.push(StackItem {
					ty: StackType::Dict,
					object_refs: object_refs
				});

				Some(PlistEvent::StartDictionary)
			},
			(_, _) => panic!("unsupported type")
		};

		Ok(result)
	}
}


impl<R: Read+Seek> Iterator for StreamingParser<R> {
	type Item = PlistEvent;

	fn next(&mut self) -> Option<PlistEvent> {
		match self.read_next() {
			Ok(result) => result,
			Err(_) => Some(PlistEvent::Error(()))
		}
	}
}


#[cfg(test)]
mod tests {
	use std::fs::File;
	use std::path::Path;

	use super::*;
	use super::super::PlistEvent;

	#[test]
	fn streaming_parser() {
		use super::super::PlistEvent::*;

		let reader = File::open(&Path::new("./tests/data/binary.plist")).unwrap();
		let streaming_parser = StreamingParser::open(reader).unwrap();
		let events: Vec<PlistEvent> = streaming_parser.collect();

		let comparison = &[
			StartDictionary,
			StringValue("Lines".to_owned()),
			StartArray,
			StringValue("It is a tale told by an idiot,".to_owned()),
			StringValue("Full of sound and fury, signifying nothing.".to_owned()),
			EndArray,
			StringValue("Height".to_owned()),
			RealValue(1.60),
			StringValue("Birthdate".to_owned()),
			IntegerValue(1564),
			StringValue("Author".to_owned()),
			StringValue("William Shakespeare".to_owned()),
			StringValue("Data".to_owned()),
			DataValue(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0]),
			EndDictionary
		];

		assert_eq!(events, comparison);
	}
}