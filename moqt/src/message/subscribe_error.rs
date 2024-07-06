use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum SubscribeErrorCode {
    #[default]
    InternalError = 0,
    InvalidRange = 1,
    RetryTrackAlias = 2,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeError {
    pub subscribe_id: u64,

    pub error_code: u64,
    pub reason_phrase: String,

    pub track_alias: u64,
}

impl Deserializer for SubscribeError {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::deserialize(r)?;

        let status_code = u64::deserialize(r)?;
        let reason_phrase = String::deserialize(r)?;

        let track_alias = u64::deserialize(r)?;

        Ok(Self {
            subscribe_id,

            error_code: status_code,
            reason_phrase,

            track_alias,
        })
    }
}

impl Serializer for SubscribeError {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.error_code.serialize(w)?;
        l += self.reason_phrase.serialize(w)?;

        l += self.track_alias.serialize(w)?;

        Ok(l)
    }
}
