use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut, Bytes};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct GroupHeader {
    pub subscribe_id: u64,
    pub track_alias: u64,
    pub group_id: u64,
    pub object_send_order: u64,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct GroupObject {
    pub object_id: u64,
    pub object_payload_length: u64,
    pub object_status: Option<u64>,
    pub object_payload: Bytes,
}

impl Decodable for GroupHeader {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        Ok(Self {
            subscribe_id: u64::decode(r)?,
            track_alias: u64::decode(r)?,
            group_id: u64::decode(r)?,
            object_send_order: u64::decode(r)?,
        })
    }
}

impl Encodable for GroupHeader {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;
        l += self.track_alias.encode(w)?;
        l += self.group_id.encode(w)?;
        l += self.object_send_order.encode(w)?;
        Ok(l)
    }
}
