use crate::message::{Role, Version};
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ServerSetup {
    pub supported_version: Version,
    pub role: Role,
}

impl Deserializer for ServerSetup {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (supported_version, svl) = Version::deserialize(r)?;

        let (mut parameters, pl) = Parameters::deserialize(r)?;
        let role: Role = parameters
            .remove(ParameterKey::Role)
            .ok_or(Error::ErrMissingParameter)?;

        Ok((
            Self {
                supported_version,
                role,
            },
            svl + pl,
        ))
    }
}

impl Serializer for ServerSetup {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_version.serialize(w)?;

        let mut parameters = Parameters::new();
        parameters.insert(ParameterKey::Role, self.role)?;
        l += parameters.serialize(w)?;
        Ok(l)
    }
}
