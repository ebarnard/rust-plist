use byteorder::{BigEndian, ReadBytesExt};
use byteorder::Error as ByteorderError;
use chrono::{TimeZone, UTC};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::string::FromUtf16Error;

use {ParserError, ParserResult, PlistEvent};

impl From<ByteorderError> for ParserError {
	fn from(err: ByteorderError) -> ParserError {
		match err {
			ByteorderError::UnexpectedEOF => ParserError::UnexpectedEof,
			ByteorderError::Io(err) => ParserError::Io(err)
		}
	}
}

impl From<FromUtf16Error> for ParserError {
	fn from(_: FromUtf16Error) -> ParserError {
		ParserError::InvalidData
	}
}

struct StackItem {
	object_refs: Vec<u64>,
	ty: StackType
}

enum StackType {
	Array,
	Dict,
	Root
}

enum ParserState {
    ExpectValue,
    ExpectKey
}

/// https://opensource.apple.com/source/CF/CF-550/CFBinaryPList.c
/// https://hg.python.org/cpython/file/3.4/Lib/plistlib.py
pub struct StreamingParser<R> {
	stack: Vec<StackItem>,
	object_offsets: Vec<u64>,
	reader: R,
	ref_size: u8,
	finished: bool,
	state: ParserState
}

impl<R: Read+Seek> StreamingParser<R> {
	pub fn new(reader: R) -> StreamingParser<R> {
		StreamingParser {
			stack: Vec::new(),
			object_offsets: Vec::new(),
			reader: reader,
			ref_size: 0,
			finished: false,
			state: ParserState::ExpectValue
		}
	}

	fn read_trailer(&mut self) -> ParserResult<()> {
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

	fn read_ints(&mut self, len: u64, size: u8) -> ParserResult<Vec<u64>> {
		let mut ints = Vec::with_capacity(len as usize);
		// TODO: Is the match hoisted out of the loop?
		for _ in 0..len {
			match size {
				1 => ints.push(try!(self.reader.read_u8()) as u64),
				2 => ints.push(try!(self.reader.read_u16::<BigEndian>()) as u64),
				4 => ints.push(try!(self.reader.read_u32::<BigEndian>()) as u64),
				8 => ints.push(try!(self.reader.read_u64::<BigEndian>()) as u64),
				_ => return Err(ParserError::InvalidData)
			}
		}
		Ok(ints)
	}

	fn read_refs(&mut self, len: u64) -> ParserResult<Vec<u64>> {
		let ref_size = self.ref_size;
		self.read_ints(len, ref_size)
	}

	fn read_object_len(&mut self, len: u8) -> ParserResult<u64> {
		if (len & 0xf) == 0xf {
			let len_power_of_two = try!(self.reader.read_u8()) & 0x3;
			Ok(match len_power_of_two {
				0 => try!(self.reader.read_u8()) as u64,
				1 => try!(self.reader.read_u16::<BigEndian>()) as u64,
				2 => try!(self.reader.read_u32::<BigEndian>()) as u64,
				3 => try!(self.reader.read_u64::<BigEndian>()),
				_ => return Err(ParserError::InvalidData)
			})
		} else {
			Ok(len as u64)
		}
	}

	fn read_data(&mut self, len: u64) -> ParserResult<Vec<u8>> {
		let mut data = vec![0; len as usize];
		let mut total_read = 0usize;

		while (total_read as u64) < len {
			let read = try!(self.reader.read(&mut data[total_read..]));
			if read == 0 {
				return Err(ParserError::UnexpectedEof);
			}
			total_read += read;
		}

		Ok(data)
	}

	fn seek_to_object(&mut self, object_ref: u64) -> ParserResult<u64> {
		let offset = *&self.object_offsets[object_ref as usize];
		let pos = try!(self.reader.seek(SeekFrom::Start(offset)));
		Ok(pos)
	}

        fn read_ascii_string(&mut self, n: u8) -> ParserResult<String> {
		let len = try!(self.read_object_len(n));
		let raw = try!(self.read_data(len));
		Ok(String::from_utf8(raw).unwrap())
        }

        fn read_unicode_string(&mut self, n: u8) -> ParserResult<String> {
		let len = try!(self.read_object_len(n));
		let raw = try!(self.read_data(len));
		let mut cursor = Cursor::new(raw);

		let mut raw_utf16 = Vec::with_capacity(len as usize / 2);
		while cursor.position() < len {
			raw_utf16.push(try!(cursor.read_u16::<BigEndian>()))
		}

		Ok(try!(String::from_utf16(&raw_utf16)))
        }

	fn read_next(&mut self) -> ParserResult<Option<PlistEvent>> {
		if self.ref_size == 0 {
			// Initialise here rather than in new
			try!(self.read_trailer());
			self.state = ParserState::ExpectValue;
			return Ok(Some(PlistEvent::StartPlist))
		}

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
				self.state = ParserState::ExpectKey;
				match item.ty {
					StackType::Array => return Ok(Some(PlistEvent::EndArray)),
					StackType::Dict => return Ok(Some(PlistEvent::EndDictionary)),
					StackType::Root => return Ok(Some(PlistEvent::EndPlist))
				}
			}
		}

		let token = try!(self.reader.read_u8());
		let ty = (token & 0xf0) >> 4;
		let size = token & 0x0f;

		let result = match self.state {
			ParserState::ExpectKey => match (ty, size) {
				(0x5, n) => { // ASCII string
					self.state = ParserState::ExpectValue;
					Some(PlistEvent::Key(try!(self.read_ascii_string(n))))
				},
				(0x6, n) => { // UTF-16 string
					self.state = ParserState::ExpectValue;
					Some(PlistEvent::Key(try!(self.read_unicode_string(n))))
				},
				_ => return Err(ParserError::KeyExpected),
			},
			ParserState::ExpectValue => {
				let result = match (ty, size) {
					(0x0, 0x00) => return Err(ParserError::UnsupportedType), // null
					(0x0, 0x08) => Some(PlistEvent::BooleanValue(false)),
					(0x0, 0x09) => Some(PlistEvent::BooleanValue(true)),
					(0x0, 0x0f) => return Err(ParserError::UnsupportedType), // fill
					(0x1, 0) => Some(PlistEvent::IntegerValue(try!(self.reader.read_u8()) as i64)),
					(0x1, 1) => Some(PlistEvent::IntegerValue(try!(self.reader.read_u16::<BigEndian>()) as i64)),
					(0x1, 2) => Some(PlistEvent::IntegerValue(try!(self.reader.read_u32::<BigEndian>()) as i64)),
					(0x1, 3) => Some(PlistEvent::IntegerValue(try!(self.reader.read_i64::<BigEndian>()))),
					(0x1, 4) => return Err(ParserError::UnsupportedType), // 128 bit int
					(0x1, _) => return Err(ParserError::UnsupportedType), // variable length int
					(0x2, 2) => Some(PlistEvent::RealValue(try!(self.reader.read_f32::<BigEndian>()) as f64)),
					(0x2, 3) => Some(PlistEvent::RealValue(try!(self.reader.read_f64::<BigEndian>()))),
					(0x2, _) => return Err(ParserError::UnsupportedType), // odd length float
					(0x3, 3) => { // Date
						// Seconds since 1/1/2001 00:00:00
						let timestamp = try!(self.reader.read_f64::<BigEndian>());

						let secs = timestamp.floor();
						let subsecs = timestamp - secs;

						let int_secs = (secs as i64) + (31 * 365 + 8) * 86400;
						let int_nanos = (subsecs * 1_000_000_000f64) as u32;

						Some(PlistEvent::DateValue(UTC.timestamp(int_secs, int_nanos)))
					}
					(0x4, n) => { // Data
						let len = try!(self.read_object_len(n));
						Some(PlistEvent::DataValue(try!(self.read_data(len))))
					},
					(0x5, n) => // ASCII string
						Some(PlistEvent::StringValue(try!(self.read_ascii_string(n)))),
					(0x6, n) => // UTF-16 string
						Some(PlistEvent::StringValue(try!(self.read_unicode_string(n)))),
					(0xa, n) => { // Array
						let len = try!(self.read_object_len(n));
						let mut object_refs = try!(self.read_refs(len));
						// Reverse so we can pop off the end of the stack in order
						object_refs.reverse();

						self.stack.push(StackItem {
							ty: StackType::Array,
							object_refs: object_refs
						});

						Some(PlistEvent::StartArray(Some(len)))
					},
					(0xd, n) => { // Dict
						let len = try!(self.read_object_len(n));
						let key_refs = try!(self.read_refs(len));
						let value_refs = try!(self.read_refs(len));

						let len = len as usize;

						let mut object_refs = Vec::with_capacity(len * 2);

						for i in 1..len+1 {
							// Reverse so we can pop off the end of the stack in order
							object_refs.push(value_refs[len - i]);
							object_refs.push(key_refs[len - i]);
						}

						self.stack.push(StackItem {
							ty: StackType::Dict,
							object_refs: object_refs
						});

						Some(PlistEvent::StartDictionary(Some(len as u64)))
					},
					(_, _) => return Err(ParserError::InvalidData)
				};
				match self.stack.last() {
					Some(t) => {
						match t.ty {
							StackType::Array => (),
							_ => self.state = ParserState::ExpectKey
						}
					},
					_ => self.state = ParserState::ExpectKey
				}
				result
			}
		};

		Ok(result)
	}
}

impl<R: Read+Seek> Iterator for StreamingParser<R> {
	type Item = ParserResult<PlistEvent>;

	fn next(&mut self) -> Option<ParserResult<PlistEvent>> {
		if self.finished {
			None
		} else {
			match self.read_next() {
				Ok(Some(event)) => Some(Ok(event)),
				Err(err) => {
					self.finished = true;
					Some(Err(err))
				},
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
		let streaming_parser = StreamingParser::new(reader);
		let events: Vec<PlistEvent> = streaming_parser.map(|e| e.unwrap()).collect();

		let comparison = &[
			StartPlist,
			StartDictionary(Some(6)),
			Key("Lines".to_owned()),
			StartArray(Some(2)),
			StringValue("It is a tale told by an idiot,".to_owned()),
			StringValue("Full of sound and fury, signifying nothing.".to_owned()),
			EndArray,
			Key("Death".to_owned()),
			IntegerValue(1564),
			Key("Height".to_owned()),
			RealValue(1.60),
			Key("Birthdate".to_owned()),
			DateValue(UTC.ymd(1981, 05, 16).and_hms(11, 32, 06)),
			Key("Author".to_owned()),
			StringValue("William Shakespeare".to_owned()),
			Key("Data".to_owned()),
			DataValue(vec![0, 0, 0, 190, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0]),
			EndDictionary,
			EndPlist
		];

		assert_eq!(events, comparison);
	}
}
