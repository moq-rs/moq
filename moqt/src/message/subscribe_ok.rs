use crate::message::FullSequence;
use crate::{Deserializer, Serializer, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeOk {
    pub subscribe_id: u64,

    pub expires: u64,

    pub largest_group_object: Option<FullSequence>,
}

impl Deserializer for SubscribeOk {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::deserialize(r)?;

        let expires = u64::deserialize(r)?;

        let largest_group_object = if bool::deserialize(r)? {
            Some(FullSequence::deserialize(r)?)
        } else {
            None
        };

        Ok(Self {
            subscribe_id,

            expires,

            largest_group_object,
        })
    }
}

impl Serializer for SubscribeOk {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.expires.serialize(w)?;

        l += if let Some(largest_group_object) = self.largest_group_object.as_ref() {
            true.serialize(w)? + largest_group_object.serialize(w)?
        } else {
            false.serialize(w)?
        };

        Ok(l)
    }
}
