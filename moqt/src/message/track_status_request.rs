use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackStatusRequest {
    pub track_namespace: String,
    pub track_name: String,
}

impl Deserializer for TrackStatusRequest {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        let (track_name, tnl) = String::deserialize(r)?;
        Ok((
            Self {
                track_namespace,
                track_name,
            },
            tnsl + tnl,
        ))
    }
}

impl Serializer for TrackStatusRequest {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;
        l += self.track_name.serialize(w)?;
        Ok(l)
    }
}
