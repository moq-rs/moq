use crate::message::GroupObjectPair;
use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeOk {
    pub subscribe_id: u64,

    pub expires: u64,

    pub largest_group_object: Option<GroupObjectPair>,
}

impl Decodable for SubscribeOk {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let expires = u64::decode(r)?;

        let group_object_pair = if bool::decode(r)? {
            Some(GroupObjectPair::decode(r)?)
        } else {
            None
        };

        Ok(Self {
            subscribe_id,

            expires,

            largest_group_object: group_object_pair,
        })
    }
}

impl Encodable for SubscribeOk {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.expires.encode(w)?;

        l += if let Some(group_object_pair) = self.largest_group_object.as_ref() {
            true.encode(w)? + group_object_pair.encode(w)?
        } else {
            false.encode(w)?
        };

        Ok(l)
    }
}
