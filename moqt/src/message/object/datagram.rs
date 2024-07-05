use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut, Bytes};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct DatagramHeader {
    pub subscribe_id: u64,
    pub track_alias: u64,
    pub group_id: u64,
    pub object_id: u64,
    pub object_send_order: u64,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct DatagramObject {
    pub object_status: u64,
    pub object_payload: Bytes,
}

impl Decodable for DatagramHeader {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        Ok(Self {
            subscribe_id: u64::decode(r)?,
            track_alias: u64::decode(r)?,
            group_id: u64::decode(r)?,
            object_id: u64::decode(r)?,
            object_send_order: u64::decode(r)?,
        })
    }
}

impl Encodable for DatagramHeader {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;
        l += self.track_alias.encode(w)?;
        l += self.group_id.encode(w)?;
        l += self.object_id.encode(w)?;
        l += self.object_send_order.encode(w)?;
        Ok(l)
    }
}