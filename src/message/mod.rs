use crate::{Decodable, Encodable, Error, Result};
use bytes::{Buf, BufMut};

mod announce;
mod announce_cancel;
mod announce_error;
mod announce_ok;
mod subscribe;
mod subscribe_done;
mod subscribe_error;
mod subscribe_ok;
mod subscribe_update;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum MessageType {
    #[default]
    ObjectStream = 0x0,
    ObjectDatagram = 0x1,
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

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct GroupObjectPair {
    pub group_id: u64,
    pub object_id: u64,
}

impl Decodable for GroupObjectPair {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let group_id = u64::decode(r)?;
        let object_id = u64::decode(r)?;
        Ok(Self {
            group_id,
            object_id,
        })
    }
}

impl Encodable for GroupObjectPair {
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
    LatestObject,                                    // = 0x2,
    AbsoluteStart(GroupObjectPair),                  // = 0x3,
    AbsoluteRange(GroupObjectPair, GroupObjectPair), // = 0x4,
}

impl Decodable for FilterType {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let v = u64::decode(r)?;
        match v {
            0x1 => Ok(FilterType::LatestGroup),
            0x2 => Ok(FilterType::LatestObject),
            0x3 => {
                let start = GroupObjectPair::decode(r)?;
                Ok(FilterType::AbsoluteStart(start))
            }
            0x4 => {
                let start = GroupObjectPair::decode(r)?;
                let end = GroupObjectPair::decode(r)?;
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
