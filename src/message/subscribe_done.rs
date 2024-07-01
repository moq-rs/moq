use crate::message::GroupObjectPair;
use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeDone {
    pub subscribe_id: u64,

    pub status_code: u64,
    pub reason_phrase: String,

    pub group_object_pair: Option<GroupObjectPair>,
}

impl Decodable for SubscribeDone {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let status_code = u64::decode(r)?;
        let reason_phrase = String::decode(r)?;

        let group_object_pair = if bool::decode(r)? {
            Some(GroupObjectPair::decode(r)?)
        } else {
            None
        };

        Ok(Self {
            subscribe_id,

            status_code,
            reason_phrase,

            group_object_pair,
        })
    }
}

impl Encodable for SubscribeDone {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.status_code.encode(w)?;
        l += self.reason_phrase.encode(w)?;

        l += if let Some(group_object_pair) = self.group_object_pair.as_ref() {
            true.encode(w)? + group_object_pair.encode(w)?
        } else {
            false.encode(w)?
        };

        Ok(l)
    }
}
