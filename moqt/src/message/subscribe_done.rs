use crate::message::FullSequence;
use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeDone {
    pub subscribe_id: u64,

    pub status_code: u64,
    pub reason_phrase: String,

    pub final_group_object: Option<FullSequence>,
}

impl Decodable for SubscribeDone {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let status_code = u64::decode(r)?;
        let reason_phrase = String::decode(r)?;

        let group_object_pair = if bool::decode(r)? {
            Some(FullSequence::decode(r)?)
        } else {
            None
        };

        Ok(Self {
            subscribe_id,

            status_code,
            reason_phrase,

            final_group_object: group_object_pair,
        })
    }
}

impl Encodable for SubscribeDone {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.status_code.encode(w)?;
        l += self.reason_phrase.encode(w)?;

        l += if let Some(group_object_pair) = self.final_group_object.as_ref() {
            true.encode(w)? + group_object_pair.encode(w)?
        } else {
            false.encode(w)?
        };

        Ok(l)
    }
}
