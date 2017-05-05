#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate plist;

use std::io::Cursor;
use plist::Plist;
use plist::binary::EventReader;

fuzz_target!(|data: &[u8]| {
    let cursor = Cursor::new(data);
    let reader = EventReader::new(cursor);
    let _ = Plist::from_events(reader);
});
