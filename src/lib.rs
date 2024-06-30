#![warn(rust_2018_idioms)]
#![allow(dead_code)]

mod codable;
mod error;
mod message;
mod object;
mod session;

pub use error::{Error, Result};
