#![no_main]
extern crate libfuzzer_sys;
extern crate plist;

use std::io::Cursor;
use plist::Plist;

#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    let cursor = Cursor::new(data);
    let _ = Plist::read(cursor);
}
