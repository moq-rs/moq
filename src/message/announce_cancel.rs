use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

pub struct AnnounceCancel {
    pub track_namespace: String,
}

impl Decodable for AnnounceCancel {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        Ok(Self { track_namespace })
    }
}

impl Encodable for AnnounceCancel {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.track_namespace.encode(w)
    }
}
