#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate plist;

use plist::stream::AsciiReader;
use plist::Value;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let cursor = Cursor::new(data);
    let reader = AsciiReader::new(cursor);
    let _ = Value::from_events(reader);
});
