use crate::message::FullSequence;
use crate::{Deserializer, Result, Serializer};
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
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;

        let (status_code, scl) = u64::deserialize(r)?;
        let (reason_phrase, rpl) = String::deserialize(r)?;

        let (exist, el) = bool::deserialize(r)?;
        let mut tl = sil + scl + rpl + el;
        let final_group_object = if exist {
            let (final_group_object, fgol) = FullSequence::deserialize(r)?;
            tl += fgol;
            Some(final_group_object)
        } else {
            None
        };

        Ok((
            Self {
                subscribe_id,

                status_code,
                reason_phrase,

                final_group_object,
            },
            tl,
        ))
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
