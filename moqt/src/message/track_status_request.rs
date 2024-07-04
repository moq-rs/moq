use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackStatusRequest {
    pub track_namespace: String,
    pub track_name: String,
}

impl Decodable for TrackStatusRequest {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        let track_name = String::decode(r)?;
        Ok(Self {
            track_namespace,
            track_name,
        })
    }
}

impl Encodable for TrackStatusRequest {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.encode(w)?;
        l += self.track_name.encode(w)?;
        Ok(l)
    }
}
