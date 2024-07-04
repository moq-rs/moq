use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnAnnounce {
    pub track_namespace: String,
}

impl Decodable for UnAnnounce {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        Ok(Self { track_namespace })
    }
}

impl Encodable for UnAnnounce {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.track_namespace.encode(w)
    }
}
