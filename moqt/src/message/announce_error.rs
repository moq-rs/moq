use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum AnnounceErrorCode {
    #[default]
    InternalError = 0,
    AnnounceNotSupported = 1,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceError {
    pub track_namespace: String,
    pub error_code: u64,
    pub reason_phrase: String,
}

impl Decodable for AnnounceError {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        let error_code = u64::decode(r)?;
        let reason_phrase = String::decode(r)?;
        Ok(Self {
            track_namespace,
            error_code,
            reason_phrase,
        })
    }
}

impl Encodable for AnnounceError {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.encode(w)?;
        l += self.error_code.encode(w)?;
        l += self.reason_phrase.encode(w)?;
        Ok(l)
    }
}
