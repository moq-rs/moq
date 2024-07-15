use crate::message::announce::Announce;
use crate::message::announce_cancel::AnnounceCancel;
use crate::message::announce_error::AnnounceError;
use crate::message::announce_ok::AnnounceOk;
use crate::message::client_setup::ClientSetup;
use crate::message::go_away::GoAway;
use crate::message::message_parser::ParserErrorCode;
use crate::message::object::ObjectForwardingPreference;
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
use bytes::{Buf, BufMut};

pub mod announce;
pub mod announce_cancel;
pub mod announce_error;
pub mod announce_ok;
pub mod client_setup;
pub mod go_away;
pub mod message_framer;
pub mod message_parser;
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

#[cfg(test)]
mod message_framer_test;
#[cfg(test)]
mod message_parser_test;
#[cfg(test)]
mod message_test;

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

impl MessageType {
    pub fn is_object_message(&self) -> bool {
        *self == MessageType::ObjectStream
            || *self == MessageType::ObjectDatagram
            || *self == MessageType::StreamHeaderTrack
            || *self == MessageType::StreamHeaderGroup
    }

    pub fn is_object_without_payload_length(&self) -> bool {
        *self == MessageType::ObjectStream || *self == MessageType::ObjectDatagram
    }

    pub fn get_object_forwarding_preference(&self) -> Result<ObjectForwardingPreference> {
        match *self {
            MessageType::ObjectStream => Ok(ObjectForwardingPreference::Object),
            MessageType::ObjectDatagram => Ok(ObjectForwardingPreference::Datagram),
            MessageType::StreamHeaderTrack => Ok(ObjectForwardingPreference::Track),
            MessageType::StreamHeaderGroup => Ok(ObjectForwardingPreference::Group),
            _ => Err(Error::ErrInvalidMessageType(*self as u64)),
        }
    }
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
            _ => Err(Error::ErrParseError(
                ParserErrorCode::ProtocolViolation,
                format!("Unknown message type 0x{:x}", value),
            )),
        }
    }
}

impl Deserializer for MessageType {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (v, l) = u64::deserialize(r)?;
        let t = v.try_into()?;
        Ok((t, l))
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
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        let (track_name, tnl) = String::deserialize(r)?;
        Ok((
            Self {
                track_namespace,
                track_name,
            },
            tnsl + tnl,
        ))
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
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (group_id, gil) = u64::deserialize(r)?;
        let (object_id, oil) = u64::deserialize(r)?;
        Ok((
            Self {
                group_id,
                object_id,
            },
            gil + oil,
        ))
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

impl FilterType {
    pub fn value(&self) -> u8 {
        match self {
            FilterType::LatestGroup => 0x1,
            FilterType::LatestObject => 0x2,
            FilterType::AbsoluteStart(_) => 0x3,
            FilterType::AbsoluteRange(_, _) => 0x4,
        }
    }
}

impl Deserializer for FilterType {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (v, vl) = u64::deserialize(r)?;
        match v {
            0x1 => Ok((FilterType::LatestGroup, vl)),
            0x2 => Ok((FilterType::LatestObject, vl)),
            0x3 => {
                let (start, sl) = FullSequence::deserialize(r)?;
                Ok((FilterType::AbsoluteStart(start), vl + sl))
            }
            0x4 => {
                let (start, sl) = FullSequence::deserialize(r)?;
                let (mut end, el) = FullSequence::deserialize(r)?;
                if end.object_id == 0 {
                    end.object_id = u64::MAX;
                } else {
                    end.object_id -= 1;
                }
                if end.group_id < start.group_id {
                    Err(Error::ErrParseError(
                        ParserErrorCode::ProtocolViolation,
                        "End group is less than start group".to_string(),
                    ))
                } else if end.group_id == start.group_id && end.object_id < start.object_id {
                    Err(Error::ErrParseError(
                        ParserErrorCode::ProtocolViolation,
                        "End object comes before start object".to_string(),
                    ))
                } else {
                    Ok((FilterType::AbsoluteRange(start, end), vl + sl + el))
                }
            }
            _ => Err(Error::ErrInvalidFilterType(v)),
        }
    }
}

impl Serializer for FilterType {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        match *self {
            FilterType::LatestGroup => 0x1u64.serialize(w),
            FilterType::LatestObject => 0x2u64.serialize(w),
            FilterType::AbsoluteStart(start) => {
                let mut l = 0x3u64.serialize(w)?;
                l += start.serialize(w)?;
                Ok(l)
            }
            FilterType::AbsoluteRange(start, mut end) => {
                if end.group_id < start.group_id {
                    return Err(Error::ErrFrameError(
                        "End group is less than start group".to_string(),
                    ));
                } else if end.group_id == start.group_id && end.object_id < start.object_id {
                    return Err(Error::ErrFrameError(
                        "End object comes before start object".to_string(),
                    ));
                }

                let mut l = 0x4u64.serialize(w)?;
                l += start.serialize(w)?;
                if end.object_id == u64::MAX {
                    end.object_id = 0;
                } else {
                    end.object_id += 1;
                }
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
    Unsupported(u32),
}

impl From<u64> for Version {
    fn from(value: u64) -> Self {
        match value {
            0xff000000 => Version::Draft00,
            0xff000001 => Version::Draft01,
            0xff000002 => Version::Draft02,
            0xff000003 => Version::Draft03,
            0xff000004 => Version::Draft04,
            _ => Version::Unsupported(value as u32),
        }
    }
}

impl Deserializer for Version {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (v, vl) = u64::deserialize(r)?;
        let version = v.into();
        Ok((version, vl))
    }
}

impl Serializer for Version {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let value: u64 = match *self {
            Version::Draft00 => 0xff000000,
            Version::Draft01 => 0xff000001,
            Version::Draft02 => 0xff000002,
            Version::Draft03 => 0xff000003,
            Version::Draft04 => 0xff000004,
            Version::Unsupported(value) => value as u64,
        };
        value.serialize(w)
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
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (v, vl) = u64::deserialize(r)?;
        let role = v.try_into()?;
        Ok((role, vl))
    }
}

impl Serializer for Role {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).serialize(w)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ControlMessage {
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

impl Deserializer for ControlMessage {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (message_type, mtl) = MessageType::deserialize(r)?;
        match message_type {
            MessageType::ObjectStream
            | MessageType::StreamHeaderTrack
            | MessageType::StreamHeaderGroup
            | MessageType::ObjectDatagram => Err(Error::ErrInvalidMessageType(message_type as u64)),
            MessageType::SubscribeUpdate => {
                let (m, ml) = SubscribeUpdate::deserialize(r)?;
                Ok((ControlMessage::SubscribeUpdate(m), mtl + ml))
            }
            MessageType::Subscribe => {
                let (m, ml) = Subscribe::deserialize(r)?;
                Ok((ControlMessage::Subscribe(m), mtl + ml))
            }
            MessageType::SubscribeOk => {
                let (m, ml) = SubscribeOk::deserialize(r)?;
                Ok((ControlMessage::SubscribeOk(m), mtl + ml))
            }
            MessageType::SubscribeError => {
                let (m, ml) = SubscribeError::deserialize(r)?;
                Ok((ControlMessage::SubscribeError(m), mtl + ml))
            }
            MessageType::Announce => {
                let (m, ml) = Announce::deserialize(r)?;
                Ok((ControlMessage::Announce(m), mtl + ml))
            }
            MessageType::AnnounceOk => {
                let (m, ml) = AnnounceOk::deserialize(r)?;
                Ok((ControlMessage::AnnounceOk(m), mtl + ml))
            }
            MessageType::AnnounceError => {
                let (m, ml) = AnnounceError::deserialize(r)?;
                Ok((ControlMessage::AnnounceError(m), mtl + ml))
            }
            MessageType::UnAnnounce => {
                let (m, ml) = UnAnnounce::deserialize(r)?;
                Ok((ControlMessage::UnAnnounce(m), mtl + ml))
            }
            MessageType::UnSubscribe => {
                let (m, ml) = UnSubscribe::deserialize(r)?;
                Ok((ControlMessage::UnSubscribe(m), mtl + ml))
            }
            MessageType::SubscribeDone => {
                let (m, ml) = SubscribeDone::deserialize(r)?;
                Ok((ControlMessage::SubscribeDone(m), mtl + ml))
            }
            MessageType::AnnounceCancel => {
                let (m, ml) = AnnounceCancel::deserialize(r)?;
                Ok((ControlMessage::AnnounceCancel(m), mtl + ml))
            }
            MessageType::TrackStatusRequest => {
                let (m, ml) = TrackStatusRequest::deserialize(r)?;
                Ok((ControlMessage::TrackStatusRequest(m), mtl + ml))
            }
            MessageType::TrackStatus => {
                let (m, ml) = TrackStatus::deserialize(r)?;
                Ok((ControlMessage::TrackStatus(m), mtl + ml))
            }
            MessageType::GoAway => {
                let (m, ml) = GoAway::deserialize(r)?;
                Ok((ControlMessage::GoAway(m), mtl + ml))
            }
            MessageType::ClientSetup => {
                let (m, ml) = ClientSetup::deserialize(r)?;
                Ok((ControlMessage::ClientSetup(m), mtl + ml))
            }
            MessageType::ServerSetup => {
                let (m, ml) = ServerSetup::deserialize(r)?;
                Ok((ControlMessage::ServerSetup(m), mtl + ml))
            }
        }
    }
}

impl Serializer for ControlMessage {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        match self {
            ControlMessage::SubscribeUpdate(subscribe_update) => {
                let mut l = MessageType::SubscribeUpdate.serialize(w)?;
                l += subscribe_update.serialize(w)?;
                Ok(l)
            }
            ControlMessage::Subscribe(subscribe) => {
                let mut l = MessageType::Subscribe.serialize(w)?;
                l += subscribe.serialize(w)?;
                Ok(l)
            }
            ControlMessage::SubscribeOk(subscribe_ok) => {
                let mut l = MessageType::SubscribeOk.serialize(w)?;
                l += subscribe_ok.serialize(w)?;
                Ok(l)
            }
            ControlMessage::SubscribeError(subscribe_error) => {
                let mut l = MessageType::SubscribeError.serialize(w)?;
                l += subscribe_error.serialize(w)?;
                Ok(l)
            }
            ControlMessage::Announce(announce) => {
                let mut l = MessageType::Announce.serialize(w)?;
                l += announce.serialize(w)?;
                Ok(l)
            }
            ControlMessage::AnnounceOk(announce_ok) => {
                let mut l = MessageType::AnnounceOk.serialize(w)?;
                l += announce_ok.serialize(w)?;
                Ok(l)
            }
            ControlMessage::AnnounceError(announce_error) => {
                let mut l = MessageType::AnnounceError.serialize(w)?;
                l += announce_error.serialize(w)?;
                Ok(l)
            }
            ControlMessage::UnAnnounce(unannounce) => {
                let mut l = MessageType::UnAnnounce.serialize(w)?;
                l += unannounce.serialize(w)?;
                Ok(l)
            }
            ControlMessage::UnSubscribe(unsubscribe) => {
                let mut l = MessageType::UnSubscribe.serialize(w)?;
                l += unsubscribe.serialize(w)?;
                Ok(l)
            }
            ControlMessage::SubscribeDone(subscribe_done) => {
                let mut l = MessageType::SubscribeDone.serialize(w)?;
                l += subscribe_done.serialize(w)?;
                Ok(l)
            }
            ControlMessage::AnnounceCancel(announce_cancel) => {
                let mut l = MessageType::AnnounceCancel.serialize(w)?;
                l += announce_cancel.serialize(w)?;
                Ok(l)
            }
            ControlMessage::TrackStatusRequest(track_status_request) => {
                let mut l = MessageType::TrackStatusRequest.serialize(w)?;
                l += track_status_request.serialize(w)?;
                Ok(l)
            }
            ControlMessage::TrackStatus(track_status) => {
                let mut l = MessageType::TrackStatus.serialize(w)?;
                l += track_status.serialize(w)?;
                Ok(l)
            }
            ControlMessage::GoAway(go_away) => {
                let mut l = MessageType::GoAway.serialize(w)?;
                l += go_away.serialize(w)?;
                Ok(l)
            }
            ControlMessage::ClientSetup(client_setup) => {
                let mut l = MessageType::ClientSetup.serialize(w)?;
                l += client_setup.serialize(w)?;
                Ok(l)
            }
            ControlMessage::ServerSetup(server_setup) => {
                let mut l = MessageType::ServerSetup.serialize(w)?;
                l += server_setup.serialize(w)?;
                Ok(l)
            }
        }
    }
}
