extern crate rustc_serialize;
extern crate xml;

mod reader;

pub use reader::{Parser, StreamingParser, PlistEvent};

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Plist {
	Array(Vec<Plist>),
	Dictionary(HashMap<String, Plist>),
	Boolean(bool),
	Data(Vec<u8>),
	Date(String),
	Real(f64),
	Integer(i64),
	String(String)
}

