#![warn(rust_2018_idioms)]
#![allow(dead_code)]

mod connection;
mod driver;
mod error;
mod message;
mod protocol;
mod serde;
mod session;

pub use connection::Connection;
pub use driver::{SessionDriver, SessionTransport};
pub use error::{Error, Result};
pub use protocol::{
    Command, Config as ProtocolConfig, EventIn, EventOut, Perspective as ProtocolPerspective,
    ReadInput, ReadOutput, SessionCore, StreamPurpose, WriteOutput,
};
pub use serde::{parameters::Parameters, varint::VarInt, Deserializer, Serializer};
pub use session::config::{Config as SessionConfig, Perspective as SessionPerspective};
pub use session::Session;

/// match between client and server perspective, since there may be a proxy
/// between them.
pub type StreamId = u32;
