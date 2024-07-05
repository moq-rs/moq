//TODO: no MessageType defined for SubscribeUpdate in https://www.ietf.org/archive/id/draft-ietf-moq-transport-04.html#name-messages

use crate::codable::parameters::ParameterKey;
use crate::message::FullSequence;
use crate::{Decodable, Encodable, Parameters};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeUpdate {
    pub subscribe_id: u64,

    pub start_group_object: FullSequence,
    pub end_group_object: FullSequence,

    pub authorization_info: Option<String>,
}

impl Decodable for SubscribeUpdate {
    fn decode<R: Buf>(r: &mut R) -> crate::Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let start_group_object = FullSequence::decode(r)?;
        let end_group_object = FullSequence::decode(r)?;

        let mut parameters = Parameters::decode(r)?;
        let authorization_info: Option<String> = parameters.remove(ParameterKey::AuthorizationInfo);

        Ok(Self {
            subscribe_id,

            start_group_object,
            end_group_object,

            authorization_info,
        })
    }
}

impl Encodable for SubscribeUpdate {
    fn encode<W: BufMut>(&self, w: &mut W) -> crate::Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.start_group_object.encode(w)?;
        l += self.end_group_object.encode(w)?;

        if let Some(authorization_info) = self.authorization_info.as_ref() {
            let mut parameters = Parameters::new();
            parameters.insert(
                ParameterKey::AuthorizationInfo,
                authorization_info.to_string(),
            )?;
            l += parameters.encode(w)?;
        }

        Ok(l)
    }
}
