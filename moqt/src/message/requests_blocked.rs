use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct RequestsBlocked {
    pub max_request_id: u64,
}

impl Deserializer for RequestsBlocked {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (max_request_id, len) = u64::deserialize(r)?;
        Ok((Self { max_request_id }, len))
    }
}

impl Serializer for RequestsBlocked {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.max_request_id.serialize(w)
    }
}
