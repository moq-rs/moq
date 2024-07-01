use crate::message::GroupObjectPair;
use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackStatus {
    pub track_namespace: String,
    pub track_name: String,
    pub status_code: u64,
    pub last_group_object: GroupObjectPair,
}

impl Decodable for TrackStatus {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        let track_name = String::decode(r)?;
        let status_code = u64::decode(r)?;
        let last_group_object = GroupObjectPair::decode(r)?;
        Ok(Self {
            track_namespace,
            track_name,
            status_code,
            last_group_object,
        })
    }
}

impl Encodable for TrackStatus {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.encode(w)?;
        l += self.track_name.encode(w)?;
        l += self.status_code.encode(w)?;
        l += self.last_group_object.encode(w)?;
        Ok(l)
    }
}
