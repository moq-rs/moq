use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct FetchCancel {
    pub request_id: u64,
}

impl Deserializer for FetchCancel {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (request_id, len) = u64::deserialize(r)?;
        Ok((Self { request_id }, len))
    }
}

impl Serializer for FetchCancel {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.request_id.serialize(w)
    }
}
