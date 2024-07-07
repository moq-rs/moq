use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum AnnounceErrorCode {
    #[default]
    InternalError = 0,
    AnnounceNotSupported = 1,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceErrorReason {
    pub error_code: AnnounceErrorCode,
    pub reason_phrase: String,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceError {
    pub track_namespace: String,
    pub error_code: u64,
    pub reason_phrase: String,
}

impl Deserializer for AnnounceError {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        let (error_code, ecl) = u64::deserialize(r)?;
        let (reason_phrase, rpl) = String::deserialize(r)?;
        Ok((
            Self {
                track_namespace,
                error_code,
                reason_phrase,
            },
            tnsl + ecl + rpl,
        ))
    }
}

impl Serializer for AnnounceError {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;
        l += self.error_code.serialize(w)?;
        l += self.reason_phrase.serialize(w)?;
        Ok(l)
    }
}
