#![no_main]

use libfuzzer_sys::fuzz_target;
use plist::stream::BinaryReader;
use plist::Value;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let cursor = Cursor::new(data);
    let reader = BinaryReader::new(cursor);
    let _ = Value::from_events(reader);
});
