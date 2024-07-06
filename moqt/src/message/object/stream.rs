use crate::{Deserializer, Serializer, Result};
use bytes::{Buf, BufMut, Bytes};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct StreamHeader {
    pub subscribe_id: u64,
    pub track_alias: u64,
    pub group_id: u64,
    pub object_id: u64,
    pub object_send_order: u64,
}

impl Deserializer for StreamHeader {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        Ok(Self {
            subscribe_id: u64::deserialize(r)?,
            track_alias: u64::deserialize(r)?,
            group_id: u64::deserialize(r)?,
            object_id: u64::deserialize(r)?,
            object_send_order: u64::deserialize(r)?,
        })
    }
}

impl Serializer for StreamHeader {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;
        l += self.track_alias.serialize(w)?;
        l += self.group_id.serialize(w)?;
        l += self.object_id.serialize(w)?;
        l += self.object_send_order.serialize(w)?;
        Ok(l)
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct StreamObject {
    pub object_status: u64,
    pub object_payload: Bytes,
}

impl Deserializer for StreamObject {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        Ok(Self {
            object_status: u64::deserialize(r)?,
            object_payload: Bytes::deserialize(r)?,
        })
    }
}

impl Serializer for StreamObject {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.object_status.serialize(w)?;
        l += self.object_payload.serialize(w)?;
        Ok(l)
    }
}
