use crate::message::FullSequence;
use crate::{Deserializer, Serializer, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum SubscribeDoneCode {
    #[default]
    Unsubscribed = 0x0,
    InternalError = 0x1,
    Unauthorized = 0x2,
    TrackEnded = 0x3,
    SubscriptionEnded = 0x4,
    GoingAway = 0x5,
    Expired = 0x6,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeDone {
    pub subscribe_id: u64,

    pub status_code: u64,
    pub reason_phrase: String,

    pub final_group_object: Option<FullSequence>,
}

impl Deserializer for SubscribeDone {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::deserialize(r)?;

        let status_code = u64::deserialize(r)?;
        let reason_phrase = String::deserialize(r)?;

        let group_object_pair = if bool::deserialize(r)? {
            Some(FullSequence::deserialize(r)?)
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

impl Serializer for SubscribeDone {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.status_code.serialize(w)?;
        l += self.reason_phrase.serialize(w)?;

        l += if let Some(group_object_pair) = self.final_group_object.as_ref() {
            true.serialize(w)? + group_object_pair.serialize(w)?
        } else {
            false.serialize(w)?
        };

        Ok(l)
    }
}
