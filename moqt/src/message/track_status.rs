use crate::message::FullSequence;
use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum TrackStatusCode {
    #[default]
    InProgress = 0x0,
    DoesNotExist = 0x1,
    NotYetBegun = 0x2,
    Finished = 0x3,
    StatusNotAvailable = 0x4,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackStatus {
    pub track_namespace: String,
    pub track_name: String,
    pub status_code: u64,
    pub last_group_object: FullSequence,
}

impl Deserializer for TrackStatus {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        let (track_name, tnl) = String::deserialize(r)?;
        let (status_code, scl) = u64::deserialize(r)?;
        let (last_group_object, lgol) = FullSequence::deserialize(r)?;
        Ok((
            Self {
                track_namespace,
                track_name,
                status_code,
                last_group_object,
            },
            tnsl + tnl + scl + lgol,
        ))
    }
}

impl Serializer for TrackStatus {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;
        l += self.track_name.serialize(w)?;
        l += self.status_code.serialize(w)?;
        l += self.last_group_object.serialize(w)?;
        Ok(l)
    }
}
