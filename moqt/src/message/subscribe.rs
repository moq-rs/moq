use crate::message::FilterType;
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Parameters, Result, Serializer};
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

impl Deserializer for Subscribe {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;

        let (track_alias, tal) = u64::deserialize(r)?;
        let (track_namespace, tnsl) = String::deserialize(r)?;
        let (track_name, tnl) = String::deserialize(r)?;

        let (filter_type, ftl) = FilterType::deserialize(r)?;

        let (mut parameters, pl) = Parameters::deserialize(r)?;
        let authorization_info: Option<String> = parameters.remove(ParameterKey::AuthorizationInfo);

        Ok((
            Self {
                subscribe_id,

                track_alias,
                track_namespace,
                track_name,

                filter_type,

                authorization_info,
            },
            sil + tal + tnsl + tnl + ftl + pl,
        ))
    }
}

impl Serializer for Subscribe {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.track_alias.serialize(w)?;
        l += self.track_namespace.serialize(w)?;
        l += self.track_name.serialize(w)?;

        l += self.filter_type.serialize(w)?;

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
