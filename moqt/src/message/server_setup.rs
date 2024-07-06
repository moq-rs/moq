use crate::serde::parameters::ParameterKey;
use crate::message::{Role, Version};
use crate::{Deserializer, Serializer, Error, Parameters, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ServerSetup {
    pub supported_version: Version,
    pub role: Role,
}

impl Deserializer for ServerSetup {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let supported_version = Version::deserialize(r)?;

        let mut parameters = Parameters::deserialize(r)?;
        let role: Role = parameters
            .remove(ParameterKey::Role)
            .ok_or(Error::ErrMissingParameter)?;

        Ok(Self {
            supported_version,
            role,
        })
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
