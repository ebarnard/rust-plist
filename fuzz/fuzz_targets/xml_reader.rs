#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate plist;

use std::io::Cursor;
use plist::Value;
use plist::stream::XmlReader;

fuzz_target!(|data: &[u8]| {
    let cursor = Cursor::new(data);
    let reader = XmlReader::new(cursor);
    let _ = Value::from_events(reader);
});
