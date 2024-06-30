#![warn(rust_2018_idioms)]
#![allow(dead_code)]

mod codable;
mod error;
mod params;
mod string;
mod varint;

pub use error::{Error, Result};
