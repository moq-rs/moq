//TODO: no MessageType defined for SubscribeUpdate in https://www.ietf.org/archive/id/draft-ietf-moq-transport-04.html#name-messages

use crate::message::FullSequence;
use crate::serde::parameters::ParameterKey;
use crate::Result;
use crate::{Deserializer, Parameters, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeUpdate {
    pub subscribe_id: u64,

    pub start_group_object: FullSequence,
    pub end_group_object: FullSequence,

    pub authorization_info: Option<String>,
}

impl Deserializer for SubscribeUpdate {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;

        let (start_group_object, sgol) = FullSequence::deserialize(r)?;
        let (end_group_object, egol) = FullSequence::deserialize(r)?;

        let (mut parameters, pl) = Parameters::deserialize(r)?;
        let authorization_info: Option<String> = parameters.remove(ParameterKey::AuthorizationInfo);

        Ok((
            Self {
                subscribe_id,

                start_group_object,
                end_group_object,

                authorization_info,
            },
            sil + sgol + egol + pl,
        ))
    }
}

impl Serializer for SubscribeUpdate {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.start_group_object.serialize(w)?;
        l += self.end_group_object.serialize(w)?;

        if let Some(authorization_info) = self.authorization_info.as_ref() {
            let mut parameters = Parameters::new();
            parameters.insert(
                ParameterKey::AuthorizationInfo,
                authorization_info.to_string(),
            )?;
            l += parameters.serialize(w)?;
        }

        Ok(l)
    }
}
