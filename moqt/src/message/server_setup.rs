use crate::codable::parameters::ParameterKey;
use crate::message::{Role, Version};
use crate::{Decodable, Encodable, Error, Parameters, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ServerSetup {
    pub supported_version: Version,
    pub role: Role,
}

impl Decodable for ServerSetup {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let supported_version = Version::decode(r)?;

        let mut parameters = Parameters::decode(r)?;
        let role: Role = parameters
            .remove(ParameterKey::Role)
            .ok_or(Error::ErrMissingParameter)?;

        Ok(Self {
            supported_version,
            role,
        })
    }
}

impl Encodable for ServerSetup {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_version.encode(w)?;

        let mut parameters = Parameters::new();
        parameters.insert(ParameterKey::Role, self.role)?;
        l += parameters.encode(w)?;
        Ok(l)
    }
}
