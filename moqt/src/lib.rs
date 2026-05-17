#![warn(rust_2018_idioms)]
#![allow(dead_code)]

mod connection;
mod driver;
mod error;
mod message;
mod protocol;
mod serde;
mod session;

pub use error::{Error, Result};
pub use serde::{parameters::Parameters, varint::VarInt, Deserializer, Serializer};

/// match between client and server perspective, since there may be a proxy
/// between them.
pub type StreamId = u32;
