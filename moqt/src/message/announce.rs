use crate::codable::parameters::ParameterKey;
use crate::{Decodable, Encodable, Parameters, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Announce {
    pub track_namespace: String,
    pub authorization_info: Option<String>,
}

impl Decodable for Announce {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;

        let mut parameters = Parameters::decode(r)?;
        let authorization_info: Option<String> = parameters.remove(ParameterKey::AuthorizationInfo);

        Ok(Self {
            track_namespace,
            authorization_info,
        })
    }
}

impl Encodable for Announce {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.encode(w)?;

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
