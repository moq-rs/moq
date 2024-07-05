use crate::codable::parameters::ParameterKey;
use crate::message::FilterType;
use crate::{Decodable, Encodable, Parameters, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Subscribe {
    pub subscribe_id: u64,

    pub track_alias: u64,
    pub track_namespace: String,
    pub track_name: String,

    pub filter_type: FilterType,

    pub authorization_info: Option<String>,
}

impl Decodable for Subscribe {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let track_alias = u64::decode(r)?;
        let track_namespace = String::decode(r)?;
        let track_name = String::decode(r)?;

        let filter_type = FilterType::decode(r)?;

        let mut parameters = Parameters::decode(r)?;
        let authorization_info: Option<String> = parameters.remove(ParameterKey::AuthorizationInfo);

        Ok(Self {
            subscribe_id,

            track_alias,
            track_namespace,
            track_name,

            filter_type,

            authorization_info,
        })
    }
}

impl Encodable for Subscribe {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.track_alias.encode(w)?;
        l += self.track_namespace.encode(w)?;
        l += self.track_name.encode(w)?;

        l += self.filter_type.encode(w)?;

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
