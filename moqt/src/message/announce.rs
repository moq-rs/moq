use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Announce {
    pub track_namespace: String,
    pub authorization_info: Option<String>,
}

impl Deserializer for Announce {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::deserialize(r)?;

        let mut parameters = Parameters::deserialize(r)?;
        let authorization_info: Option<String> = parameters.remove(ParameterKey::AuthorizationInfo);

        Ok(Self {
            track_namespace,
            authorization_info,
        })
    }
}

impl Serializer for Announce {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;

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
