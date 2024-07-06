use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut, Bytes};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackHeader {
    pub subscribe_id: u64,
    pub track_alias: u64,
    pub object_send_order: u64,
}

impl Decodable for TrackHeader {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        Ok(Self {
            subscribe_id: u64::decode(r)?,
            track_alias: u64::decode(r)?,
            object_send_order: u64::decode(r)?,
        })
    }
}

impl Encodable for TrackHeader {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;
        l += self.track_alias.encode(w)?;
        l += self.object_send_order.encode(w)?;
        Ok(l)
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackObject {
    pub group_id: u64,
    pub object_id: u64,
    pub object_payload_length: u64,
    pub object_status: Option<u64>,
    pub object_payload: Bytes,
}

impl Decodable for TrackObject {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let group_id = u64::decode(r)?;
        let object_id = u64::decode(r)?;
        let object_payload_length = u64::decode(r)?;
        let object_status = if object_payload_length == 0 {
            Some(u64::decode(r)?)
        } else {
            None
        };

        Ok(Self {
            group_id,
            object_id,
            object_payload_length,
            object_status,
            object_payload: Bytes::decode(r)?,
        })
    }
}

impl Encodable for TrackObject {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.group_id.encode(w)?;
        l += self.object_id.encode(w)?;
        l += self.object_payload_length.encode(w)?;
        if let Some(object_status) = self.object_status.as_ref() {
            l += object_status.encode(w)?;
        }
        l += self.object_payload.encode(w)?;
        Ok(l)
    }
}
