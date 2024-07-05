use crate::codable::parameters::ParameterKey;
use crate::message::{Role, Version};
use crate::{Decodable, Encodable, Error, Parameters, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ClientSetup {
    pub supported_versions: Vec<Version>,
    pub role: Role,
    pub path: Option<String>,
}

impl Decodable for ClientSetup {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let number_supported_versions = usize::decode(r)?;
        let mut supported_versions = Vec::with_capacity(number_supported_versions);
        for _ in 0..number_supported_versions {
            supported_versions.push(Version::decode(r)?);
        }

        let mut parameters = Parameters::decode(r)?;
        let role: Role = parameters
            .remove(ParameterKey::Role)
            .ok_or(Error::ErrMissingParameter)?;
        let path: Option<String> = parameters.remove(ParameterKey::Path);

        Ok(Self {
            supported_versions,
            role,
            path,
        })
    }
}

impl Encodable for ClientSetup {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_versions.len().encode(w)?;
        for supported_version in self.supported_versions.iter() {
            l += supported_version.encode(w)?;
        }

        let mut parameters = Parameters::new();
        parameters.insert(ParameterKey::Role, self.role)?;
        if let Some(path) = self.path.as_ref() {
            parameters.insert(ParameterKey::Path, path.to_string())?;
        }
        l += parameters.encode(w)?;

        Ok(l)
    }
}
