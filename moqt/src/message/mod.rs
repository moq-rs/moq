use crate::message::announce::Announce;
use crate::message::announce_cancel::AnnounceCancel;
use crate::message::announce_error::AnnounceError;
use crate::message::announce_ok::AnnounceOk;
use crate::message::client_setup::ClientSetup;
use crate::message::go_away::GoAway;
use crate::message::object::datagram::DatagramHeader;
use crate::message::object::group::GroupHeader;
use crate::message::object::stream::StreamHeader;
use crate::message::object::track::TrackHeader;
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
use crate::{Decodable, Encodable, Error, Result};
use bytes::{Buf, BufMut};

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

impl Decodable for MessageType {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::decode(r)?;
        v.try_into()
    }
}

impl Encodable for MessageType {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).encode(w)
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Hash)]
pub struct FullTrackName {
    pub track_namespace: String,
    pub track_name: String,
}

impl Decodable for FullTrackName {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        let track_name = String::decode(r)?;
        Ok(Self {
            track_namespace,
            track_name,
        })
    }
}

impl Encodable for FullTrackName {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.encode(w)?;
        l += self.track_name.encode(w)?;
        Ok(l)
    }
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct FullSequence {
    pub group_id: u64,
    pub object_id: u64,
}

impl Decodable for FullSequence {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let group_id = u64::decode(r)?;
        let object_id = u64::decode(r)?;
        Ok(Self {
            group_id,
            object_id,
        })
    }
}

impl Encodable for FullSequence {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.group_id.encode(w)?;
        l += self.object_id.encode(w)?;
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

impl Decodable for FilterType {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::decode(r)?;
        match v {
            0x1 => Ok(FilterType::LatestGroup),
            0x2 => Ok(FilterType::LatestObject),
            0x3 => {
                let start = FullSequence::decode(r)?;
                Ok(FilterType::AbsoluteStart(start))
            }
            0x4 => {
                let start = FullSequence::decode(r)?;
                let end = FullSequence::decode(r)?;
                Ok(FilterType::AbsoluteRange(start, end))
            }
            _ => Err(Error::ErrInvalidFilterType(v)),
        }
    }
}

impl Encodable for FilterType {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        match self {
            FilterType::LatestGroup => 0x1u64.encode(w),
            FilterType::LatestObject => 0x2u64.encode(w),
            FilterType::AbsoluteStart(start) => {
                let mut l = 0x3u64.encode(w)?;
                l += start.encode(w)?;
                Ok(l)
            }
            FilterType::AbsoluteRange(start, end) => {
                let mut l = 0x4u64.encode(w)?;
                l += start.encode(w)?;
                l += end.encode(w)?;
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

impl Decodable for Version {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::decode(r)?;
        v.try_into()
    }
}

impl Encodable for Version {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).encode(w)
    }
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum Role {
    #[default]
    Publisher = 0x1,
    Subscriber = 0x2,
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

impl Decodable for Role {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::decode(r)?;
        v.try_into()
    }
}

impl Encodable for Role {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).encode(w)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Message {
    ObjectStream(StreamHeader),
    ObjectDatagram(DatagramHeader),
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
    StreamHeaderTrack(TrackHeader),
    StreamHeaderGroup(GroupHeader),
}

impl Decodable for Message {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let message_type = MessageType::decode(r)?;
        match message_type {
            MessageType::ObjectStream => Ok(Message::ObjectStream(StreamHeader::decode(r)?)),
            MessageType::ObjectDatagram => Ok(Message::ObjectDatagram(DatagramHeader::decode(r)?)),
            MessageType::SubscribeUpdate => {
                Ok(Message::SubscribeUpdate(SubscribeUpdate::decode(r)?))
            }
            MessageType::Subscribe => Ok(Message::Subscribe(Subscribe::decode(r)?)),
            MessageType::SubscribeOk => Ok(Message::SubscribeOk(SubscribeOk::decode(r)?)),
            MessageType::SubscribeError => Ok(Message::SubscribeError(SubscribeError::decode(r)?)),
            MessageType::Announce => Ok(Message::Announce(Announce::decode(r)?)),
            MessageType::AnnounceOk => Ok(Message::AnnounceOk(AnnounceOk::decode(r)?)),
            MessageType::AnnounceError => Ok(Message::AnnounceError(AnnounceError::decode(r)?)),
            MessageType::UnAnnounce => Ok(Message::UnAnnounce(UnAnnounce::decode(r)?)),
            MessageType::UnSubscribe => Ok(Message::UnSubscribe(UnSubscribe::decode(r)?)),
            MessageType::SubscribeDone => Ok(Message::SubscribeDone(SubscribeDone::decode(r)?)),
            MessageType::AnnounceCancel => Ok(Message::AnnounceCancel(AnnounceCancel::decode(r)?)),
            MessageType::TrackStatusRequest => {
                Ok(Message::TrackStatusRequest(TrackStatusRequest::decode(r)?))
            }
            MessageType::TrackStatus => Ok(Message::TrackStatus(TrackStatus::decode(r)?)),
            MessageType::GoAway => Ok(Message::GoAway(GoAway::decode(r)?)),
            MessageType::ClientSetup => Ok(Message::ClientSetup(ClientSetup::decode(r)?)),
            MessageType::ServerSetup => Ok(Message::ServerSetup(ServerSetup::decode(r)?)),
            MessageType::StreamHeaderTrack => {
                Ok(Message::StreamHeaderTrack(TrackHeader::decode(r)?))
            }
            MessageType::StreamHeaderGroup => {
                Ok(Message::StreamHeaderGroup(GroupHeader::decode(r)?))
            }
        }
    }
}

impl Encodable for Message {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        match self {
            Message::ObjectStream(stream_header) => {
                let mut l = MessageType::ObjectStream.encode(w)?;
                l += stream_header.encode(w)?;
                Ok(l)
            }
            Message::ObjectDatagram(datagram_header) => {
                let mut l = MessageType::ObjectDatagram.encode(w)?;
                l += datagram_header.encode(w)?;
                Ok(l)
            }
            Message::SubscribeUpdate(subscribe_update) => {
                let mut l = MessageType::SubscribeUpdate.encode(w)?;
                l += subscribe_update.encode(w)?;
                Ok(l)
            }
            Message::Subscribe(subscribe) => {
                let mut l = MessageType::Subscribe.encode(w)?;
                l += subscribe.encode(w)?;
                Ok(l)
            }
            Message::SubscribeOk(subscribe_ok) => {
                let mut l = MessageType::SubscribeOk.encode(w)?;
                l += subscribe_ok.encode(w)?;
                Ok(l)
            }
            Message::SubscribeError(subscribe_error) => {
                let mut l = MessageType::SubscribeError.encode(w)?;
                l += subscribe_error.encode(w)?;
                Ok(l)
            }
            Message::Announce(announce) => {
                let mut l = MessageType::Announce.encode(w)?;
                l += announce.encode(w)?;
                Ok(l)
            }
            Message::AnnounceOk(announce_ok) => {
                let mut l = MessageType::AnnounceOk.encode(w)?;
                l += announce_ok.encode(w)?;
                Ok(l)
            }
            Message::AnnounceError(announce_error) => {
                let mut l = MessageType::AnnounceError.encode(w)?;
                l += announce_error.encode(w)?;
                Ok(l)
            }
            Message::UnAnnounce(unannounce) => {
                let mut l = MessageType::UnAnnounce.encode(w)?;
                l += unannounce.encode(w)?;
                Ok(l)
            }
            Message::UnSubscribe(unsubscribe) => {
                let mut l = MessageType::UnSubscribe.encode(w)?;
                l += unsubscribe.encode(w)?;
                Ok(l)
            }
            Message::SubscribeDone(subscribe_done) => {
                let mut l = MessageType::SubscribeDone.encode(w)?;
                l += subscribe_done.encode(w)?;
                Ok(l)
            }
            Message::AnnounceCancel(announce_cancel) => {
                let mut l = MessageType::AnnounceCancel.encode(w)?;
                l += announce_cancel.encode(w)?;
                Ok(l)
            }
            Message::TrackStatusRequest(track_status_request) => {
                let mut l = MessageType::TrackStatusRequest.encode(w)?;
                l += track_status_request.encode(w)?;
                Ok(l)
            }
            Message::TrackStatus(track_status) => {
                let mut l = MessageType::TrackStatus.encode(w)?;
                l += track_status.encode(w)?;
                Ok(l)
            }
            Message::GoAway(go_away) => {
                let mut l = MessageType::GoAway.encode(w)?;
                l += go_away.encode(w)?;
                Ok(l)
            }
            Message::ClientSetup(client_setup) => {
                let mut l = MessageType::ClientSetup.encode(w)?;
                l += client_setup.encode(w)?;
                Ok(l)
            }
            Message::ServerSetup(server_setup) => {
                let mut l = MessageType::ServerSetup.encode(w)?;
                l += server_setup.encode(w)?;
                Ok(l)
            }
            Message::StreamHeaderTrack(track_header) => {
                let mut l = MessageType::StreamHeaderTrack.encode(w)?;
                l += track_header.encode(w)?;
                Ok(l)
            }
            Message::StreamHeaderGroup(group_header) => {
                let mut l = MessageType::StreamHeaderGroup.encode(w)?;
                l += group_header.encode(w)?;
                Ok(l)
            }
        }
    }
}
