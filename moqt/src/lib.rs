#![warn(rust_2018_idioms)]
#![allow(dead_code)]

mod serde;
mod error;
mod message;
mod session;

pub use serde::{parameters::Parameters, varint::VarInt, Deserializer, Serializer};
pub use error::{Error, Result};
