use crate::{Deserializer, Serializer, Result};
use bytes::{Buf, BufMut, Bytes};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackHeader {
    pub subscribe_id: u64,
    pub track_alias: u64,
    pub object_send_order: u64,
}

impl Deserializer for TrackHeader {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        Ok(Self {
            subscribe_id: u64::deserialize(r)?,
            track_alias: u64::deserialize(r)?,
            object_send_order: u64::deserialize(r)?,
        })
    }
}

impl Serializer for TrackHeader {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;
        l += self.track_alias.serialize(w)?;
        l += self.object_send_order.serialize(w)?;
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

impl Deserializer for TrackObject {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let group_id = u64::deserialize(r)?;
        let object_id = u64::deserialize(r)?;
        let object_payload_length = u64::deserialize(r)?;
        let object_status = if object_payload_length == 0 {
            Some(u64::deserialize(r)?)
        } else {
            None
        };

        Ok(Self {
            group_id,
            object_id,
            object_payload_length,
            object_status,
            object_payload: Bytes::deserialize(r)?,
        })
    }
}

impl Serializer for TrackObject {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.group_id.serialize(w)?;
        l += self.object_id.serialize(w)?;
        l += self.object_payload_length.serialize(w)?;
        if let Some(object_status) = self.object_status.as_ref() {
            l += object_status.serialize(w)?;
        }
        l += self.object_payload.serialize(w)?;
        Ok(l)
    }
}
