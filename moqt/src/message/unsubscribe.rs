use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnSubscribe {
    pub subscribe_id: u64,
}

impl Deserializer for UnSubscribe {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;
        Ok((Self { subscribe_id }, sil))
    }
}

impl Serializer for UnSubscribe {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.subscribe_id.serialize(w)
    }
}
