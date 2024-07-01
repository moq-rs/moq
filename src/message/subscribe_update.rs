//TODO: no MessageType defined for SubscribeUpdate in https://www.ietf.org/archive/id/draft-ietf-moq-transport-04.html#name-messages

use crate::message::GroupObjectPair;
use crate::{Decodable, Encodable, Parameters};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeUpdate {
    pub subscribe_id: u64,

    pub start_group_object: GroupObjectPair,
    pub end_group_object: GroupObjectPair,

    pub parameters: Parameters,
}

impl Decodable for SubscribeUpdate {
    fn decode<R: Buf>(r: &mut R) -> crate::Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let start_group_object = GroupObjectPair::decode(r)?;
        let end_group_object = GroupObjectPair::decode(r)?;

        let parameters = Parameters::decode(r)?;

        Ok(Self {
            subscribe_id,

            start_group_object,
            end_group_object,

            parameters,
        })
    }
}

impl Encodable for SubscribeUpdate {
    fn encode<W: BufMut>(&self, w: &mut W) -> crate::Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.start_group_object.encode(w)?;
        l += self.end_group_object.encode(w)?;

        l += self.parameters.encode(w)?;

        Ok(l)
    }
}
