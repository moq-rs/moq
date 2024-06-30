use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

pub struct AnnounceOk {
    pub track_namespace: String,
}

impl Decodable for AnnounceOk {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        Ok(Self { track_namespace })
    }
}

impl Encodable for AnnounceOk {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.track_namespace.encode(w)
    }
}
