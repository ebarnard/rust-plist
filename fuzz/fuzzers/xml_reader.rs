#![no_main]
extern crate libfuzzer_sys;
extern crate plist;

use std::io::Cursor;
use plist::Plist;
use plist::xml::EventReader;

#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    let cursor = Cursor::new(data);
    let reader = EventReader::new(cursor);
    let _ = Plist::from_events(reader);
}