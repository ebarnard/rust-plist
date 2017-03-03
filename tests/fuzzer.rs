extern crate plist;

use std::io::Cursor;
use plist::Plist;

#[test]
fn too_large_allocation() {
    let data = b"bplist00\"&L^^^^^^^^-^^^^^^^^^^^";
    test_fuzzer_data_err(data);
}

#[test]
fn too_large_allocation_2() {
    let data = b"bplist00;<)\x9fX\x0a<h\x0a:hhhhG:hh\x0amhhhhhhx#hhT)\x0a*";
    test_fuzzer_data_err(data);
}

fn test_fuzzer_data_err(data: &[u8]) {
    let cursor = Cursor::new(data);
    let res = Plist::read(cursor);
    assert!(res.is_err());
}
