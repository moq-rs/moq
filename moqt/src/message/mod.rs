use crate::message::announce::Announce;
use crate::message::announce_cancel::AnnounceCancel;
use crate::message::announce_error::AnnounceError;
use crate::message::announce_ok::AnnounceOk;
use crate::message::client_setup::ClientSetup;
use crate::message::go_away::GoAway;
use crate::message::object::ObjectHeader;
use crate::message::server_setup::ServerSetup;
use crate::message::subscribe::Subscribe;
use crate::message::subscribe_done::SubscribeDone;
use crate::message::subscribe_error::SubscribeError;
use crate::message::subscribe_ok::SubscribeOk;
use crate::message::subscribe_update::SubscribeUpdate;
use crate::message::track_status::TrackStatus;
use crate::message::track_status_request::TrackStatusRequest;
use crate::message::unannounce::UnAnnounce;
use crate::message::unsubscribe::UnSubscribe;
use crate::{Deserializer, Error, Result, Serializer};
use bytes::{Buf, BufMut, Bytes};

pub mod announce;
pub mod announce_cancel;
pub mod announce_error;
pub mod announce_ok;
pub mod client_setup;
pub mod go_away;
pub mod object;
pub mod server_setup;
pub mod subscribe;
pub mod subscribe_done;
pub mod subscribe_error;
pub mod subscribe_ok;
pub mod subscribe_update;
pub mod track_status;
pub mod track_status_request;
pub mod unannounce;
pub mod unsubscribe;

/// The maximum length of a message, excluding and OBJECT payload.
/// This prevents DoS attack via forcing the parser to buffer a large
/// message (OBJECT payloads are not buffered by the parser)
pub const MAX_MESSSAGE_HEADER_SIZE: usize = 2048;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum MessageType {
    #[default]
    ObjectStream = 0x0,
    ObjectDatagram = 0x1,
    SubscribeUpdate = 0x2,
    Subscribe = 0x3,
    SubscribeOk = 0x4,
    SubscribeError = 0x5,
    Announce = 0x6,
    AnnounceOk = 0x7,
    AnnounceError = 0x8,
    UnAnnounce = 0x9,
    UnSubscribe = 0xa,
    SubscribeDone = 0xb,
    AnnounceCancel = 0xc,
    TrackStatusRequest = 0xd,
    TrackStatus = 0xe,
    GoAway = 0x10,
    ClientSetup = 0x40,
    ServerSetup = 0x41,
    StreamHeaderTrack = 0x50,
    StreamHeaderGroup = 0x51,
}

impl TryFrom<u64> for MessageType {
    type Error = Error;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        match value {
            0x0 => Ok(MessageType::ObjectStream),
            0x1 => Ok(MessageType::ObjectDatagram),
            0x2 => Ok(MessageType::SubscribeUpdate),
            0x3 => Ok(MessageType::Subscribe),
            0x4 => Ok(MessageType::SubscribeOk),
            0x5 => Ok(MessageType::SubscribeError),
            0x6 => Ok(MessageType::Announce),
            0x7 => Ok(MessageType::AnnounceOk),
            0x8 => Ok(MessageType::AnnounceError),
            0x9 => Ok(MessageType::UnAnnounce),
            0xa => Ok(MessageType::UnSubscribe),
            0xb => Ok(MessageType::SubscribeDone),
            0xc => Ok(MessageType::AnnounceCancel),
            0xd => Ok(MessageType::TrackStatusRequest),
            0xe => Ok(MessageType::TrackStatus),
            0x10 => Ok(MessageType::GoAway),
            0x40 => Ok(MessageType::ClientSetup),
            0x41 => Ok(MessageType::ServerSetup),
            0x50 => Ok(MessageType::StreamHeaderTrack),
            0x51 => Ok(MessageType::StreamHeaderGroup),
            _ => Err(Error::ErrInvalidMessageType(value)),
        }
    }
}

impl Deserializer for MessageType {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::deserialize(r)?;
        v.try_into()
    }
}

impl Serializer for MessageType {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).serialize(w)
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq, PartialOrd, Hash)]
pub struct FullTrackName {
    pub track_namespace: String,
    pub track_name: String,
}

impl Deserializer for FullTrackName {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::deserialize(r)?;
        let track_name = String::deserialize(r)?;
        Ok(Self {
            track_namespace,
            track_name,
        })
    }
}

impl Serializer for FullTrackName {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;
        l += self.track_name.serialize(w)?;
        Ok(l)
    }
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Hash)]
pub struct FullSequence {
    pub group_id: u64,
    pub object_id: u64,
}

impl FullSequence {
    pub fn next(&self) -> Self {
        Self {
            group_id: self.group_id,
            object_id: self.object_id + 1,
        }
    }
}

impl Deserializer for FullSequence {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let group_id = u64::deserialize(r)?;
        let object_id = u64::deserialize(r)?;
        Ok(Self {
            group_id,
            object_id,
        })
    }
}

impl Serializer for FullSequence {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.group_id.serialize(w)?;
        l += self.object_id.serialize(w)?;
        Ok(l)
    }
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum FilterType {
    #[default]
    LatestGroup, // = 0x1,
    LatestObject,                              // = 0x2,
    AbsoluteStart(FullSequence),               // = 0x3,
    AbsoluteRange(FullSequence, FullSequence), // = 0x4,
}

impl Deserializer for FilterType {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::deserialize(r)?;
        match v {
            0x1 => Ok(FilterType::LatestGroup),
            0x2 => Ok(FilterType::LatestObject),
            0x3 => {
                let start = FullSequence::deserialize(r)?;
                Ok(FilterType::AbsoluteStart(start))
            }
            0x4 => {
                let start = FullSequence::deserialize(r)?;
                let end = FullSequence::deserialize(r)?;
                Ok(FilterType::AbsoluteRange(start, end))
            }
            _ => Err(Error::ErrInvalidFilterType(v)),
        }
    }
}

impl Serializer for FilterType {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        match self {
            FilterType::LatestGroup => 0x1u64.serialize(w),
            FilterType::LatestObject => 0x2u64.serialize(w),
            FilterType::AbsoluteStart(start) => {
                let mut l = 0x3u64.serialize(w)?;
                l += start.serialize(w)?;
                Ok(l)
            }
            FilterType::AbsoluteRange(start, end) => {
                let mut l = 0x4u64.serialize(w)?;
                l += start.serialize(w)?;
                l += end.serialize(w)?;
                Ok(l)
            }
        }
    }
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum Version {
    #[default]
    Draft00 = 0xff000000,
    Draft01 = 0xff000001,
    Draft02 = 0xff000002,
    Draft03 = 0xff000003,
    Draft04 = 0xff000004,
}

impl TryFrom<u64> for Version {
    type Error = Error;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        match value {
            0xff000000 => Ok(Version::Draft00),
            0xff000001 => Ok(Version::Draft01),
            0xff000002 => Ok(Version::Draft02),
            0xff000003 => Ok(Version::Draft03),
            0xff000004 => Ok(Version::Draft04),
            _ => Err(Error::ErrUnsupportedVersion(value)),
        }
    }
}

impl Deserializer for Version {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::deserialize(r)?;
        v.try_into()
    }
}

impl Serializer for Version {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).serialize(w)
    }
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum Role {
    Publisher = 0x1,
    Subscriber = 0x2,
    #[default]
    PubSub = 0x3,
}

impl TryFrom<u64> for Role {
    type Error = Error;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        match value {
            0x1 => Ok(Role::Publisher),
            0x2 => Ok(Role::Subscriber),
            0x3 => Ok(Role::PubSub),
            _ => Err(Error::ErrInvalidRole(value)),
        }
    }
}

impl Deserializer for Role {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::deserialize(r)?;
        v.try_into()
    }
}

impl Serializer for Role {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).serialize(w)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Message {
    ObjectStream(ObjectHeader, Bytes, bool),
    ObjectDatagram(ObjectHeader, Bytes),
    SubscribeUpdate(SubscribeUpdate),
    Subscribe(Subscribe),
    SubscribeOk(SubscribeOk),
    SubscribeError(SubscribeError),
    Announce(Announce),
    AnnounceOk(AnnounceOk),
    AnnounceError(AnnounceError),
    UnAnnounce(UnAnnounce),
    UnSubscribe(UnSubscribe),
    SubscribeDone(SubscribeDone),
    AnnounceCancel(AnnounceCancel),
    TrackStatusRequest(TrackStatusRequest),
    TrackStatus(TrackStatus),
    GoAway(GoAway),
    ClientSetup(ClientSetup),
    ServerSetup(ServerSetup),
}

impl Deserializer for Message {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let message_type = MessageType::deserialize(r)?;
        match message_type {
            MessageType::ObjectStream
            | MessageType::StreamHeaderTrack
            | MessageType::StreamHeaderGroup
            | MessageType::ObjectDatagram => Err(Error::ErrInvalidMessageType(message_type as u64)),
            MessageType::SubscribeUpdate => {
                Ok(Message::SubscribeUpdate(SubscribeUpdate::deserialize(r)?))
            }
            MessageType::Subscribe => Ok(Message::Subscribe(Subscribe::deserialize(r)?)),
            MessageType::SubscribeOk => Ok(Message::SubscribeOk(SubscribeOk::deserialize(r)?)),
            MessageType::SubscribeError => {
                Ok(Message::SubscribeError(SubscribeError::deserialize(r)?))
            }
            MessageType::Announce => Ok(Message::Announce(Announce::deserialize(r)?)),
            MessageType::AnnounceOk => Ok(Message::AnnounceOk(AnnounceOk::deserialize(r)?)),
            MessageType::AnnounceError => {
                Ok(Message::AnnounceError(AnnounceError::deserialize(r)?))
            }
            MessageType::UnAnnounce => Ok(Message::UnAnnounce(UnAnnounce::deserialize(r)?)),
            MessageType::UnSubscribe => Ok(Message::UnSubscribe(UnSubscribe::deserialize(r)?)),
            MessageType::SubscribeDone => {
                Ok(Message::SubscribeDone(SubscribeDone::deserialize(r)?))
            }
            MessageType::AnnounceCancel => {
                Ok(Message::AnnounceCancel(AnnounceCancel::deserialize(r)?))
            }
            MessageType::TrackStatusRequest => Ok(Message::TrackStatusRequest(
                TrackStatusRequest::deserialize(r)?,
            )),
            MessageType::TrackStatus => Ok(Message::TrackStatus(TrackStatus::deserialize(r)?)),
            MessageType::GoAway => Ok(Message::GoAway(GoAway::deserialize(r)?)),
            MessageType::ClientSetup => Ok(Message::ClientSetup(ClientSetup::deserialize(r)?)),
            MessageType::ServerSetup => Ok(Message::ServerSetup(ServerSetup::deserialize(r)?)),
        }
    }
}

impl Serializer for Message {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        match self {
            Message::ObjectStream(_, _, _) | Message::ObjectDatagram(_, _) => {
                Err(Error::ErrInvalidMessageType(0))
            }
            Message::SubscribeUpdate(subscribe_update) => {
                let mut l = MessageType::SubscribeUpdate.serialize(w)?;
                l += subscribe_update.serialize(w)?;
                Ok(l)
            }
            Message::Subscribe(subscribe) => {
                let mut l = MessageType::Subscribe.serialize(w)?;
                l += subscribe.serialize(w)?;
                Ok(l)
            }
            Message::SubscribeOk(subscribe_ok) => {
                let mut l = MessageType::SubscribeOk.serialize(w)?;
                l += subscribe_ok.serialize(w)?;
                Ok(l)
            }
            Message::SubscribeError(subscribe_error) => {
                let mut l = MessageType::SubscribeError.serialize(w)?;
                l += subscribe_error.serialize(w)?;
                Ok(l)
            }
            Message::Announce(announce) => {
                let mut l = MessageType::Announce.serialize(w)?;
                l += announce.serialize(w)?;
                Ok(l)
            }
            Message::AnnounceOk(announce_ok) => {
                let mut l = MessageType::AnnounceOk.serialize(w)?;
                l += announce_ok.serialize(w)?;
                Ok(l)
            }
            Message::AnnounceError(announce_error) => {
                let mut l = MessageType::AnnounceError.serialize(w)?;
                l += announce_error.serialize(w)?;
                Ok(l)
            }
            Message::UnAnnounce(unannounce) => {
                let mut l = MessageType::UnAnnounce.serialize(w)?;
                l += unannounce.serialize(w)?;
                Ok(l)
            }
            Message::UnSubscribe(unsubscribe) => {
                let mut l = MessageType::UnSubscribe.serialize(w)?;
                l += unsubscribe.serialize(w)?;
                Ok(l)
            }
            Message::SubscribeDone(subscribe_done) => {
                let mut l = MessageType::SubscribeDone.serialize(w)?;
                l += subscribe_done.serialize(w)?;
                Ok(l)
            }
            Message::AnnounceCancel(announce_cancel) => {
                let mut l = MessageType::AnnounceCancel.serialize(w)?;
                l += announce_cancel.serialize(w)?;
                Ok(l)
            }
            Message::TrackStatusRequest(track_status_request) => {
                let mut l = MessageType::TrackStatusRequest.serialize(w)?;
                l += track_status_request.serialize(w)?;
                Ok(l)
            }
            Message::TrackStatus(track_status) => {
                let mut l = MessageType::TrackStatus.serialize(w)?;
                l += track_status.serialize(w)?;
                Ok(l)
            }
            Message::GoAway(go_away) => {
                let mut l = MessageType::GoAway.serialize(w)?;
                l += go_away.serialize(w)?;
                Ok(l)
            }
            Message::ClientSetup(client_setup) => {
                let mut l = MessageType::ClientSetup.serialize(w)?;
                l += client_setup.serialize(w)?;
                Ok(l)
            }
            Message::ServerSetup(server_setup) => {
                let mut l = MessageType::ServerSetup.serialize(w)?;
                l += server_setup.serialize(w)?;
                Ok(l)
            }
        }
    }
}
