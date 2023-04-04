#![no_main]

use libfuzzer_sys::fuzz_target;
use plist::stream::XmlReader;
use plist::Value;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let cursor = Cursor::new(data);
    let reader = XmlReader::new(cursor);
    let _ = Value::from_events(reader);
});
