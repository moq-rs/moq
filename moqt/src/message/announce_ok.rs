use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceOk {
    pub track_namespace: String,
}

impl Deserializer for AnnounceOk {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::deserialize(r)?;
        Ok(Self { track_namespace })
    }
}

impl Serializer for AnnounceOk {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.track_namespace.serialize(w)
    }
}
