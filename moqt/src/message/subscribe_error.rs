use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeError {
    pub subscribe_id: u64,

    pub error_code: u64,
    pub reason_phrase: String,

    pub track_alias: u64,
}

impl Decodable for SubscribeError {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let status_code = u64::decode(r)?;
        let reason_phrase = String::decode(r)?;

        let track_alias = u64::decode(r)?;

        Ok(Self {
            subscribe_id,

            error_code: status_code,
            reason_phrase,

            track_alias,
        })
    }
}

impl Encodable for SubscribeError {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.error_code.encode(w)?;
        l += self.reason_phrase.encode(w)?;

        l += self.track_alias.encode(w)?;

        Ok(l)
    }
}
