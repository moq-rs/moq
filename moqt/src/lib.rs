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
pub use message::announce::Announce;
pub use message::announce_cancel::AnnounceCancel;
pub use message::announce_error::AnnounceError;
pub use message::announce_ok::AnnounceOk;
pub use message::client_setup::ClientSetup;
pub use message::go_away::GoAway;
pub use message::object::{ObjectForwardingPreference, ObjectHeader, ObjectStatus};
pub use message::server_setup::ServerSetup;
pub use message::subscribe::Subscribe;
pub use message::subscribe_done::SubscribeDone;
pub use message::subscribe_error::SubscribeError;
pub use message::subscribe_ok::SubscribeOk;
pub use message::subscribe_update::SubscribeUpdate;
pub use message::track_status::TrackStatus;
pub use message::track_status_request::TrackStatusRequest;
pub use message::unannounce::UnAnnounce;
pub use message::unsubscribe::UnSubscribe;
pub use message::{FilterType, FullSequence, FullTrackName, Role, Version};
pub use protocol::{
    Command, Config as ProtocolConfig, EventIn, EventOut, Perspective as ProtocolPerspective,
    ReadInput, SessionCore, StreamPurpose, WriteOutput,
};
pub use serde::{parameters::Parameters, varint::VarInt, Deserializer, Serializer};
pub use session::config::{Config as SessionConfig, Perspective as SessionPerspective};
pub use session::remote_track::RemoteTrackOnObjectFragment;
pub use session::Session;

/// match between client and server perspective, since there may be a proxy
/// between them.
pub type StreamId = u32;
