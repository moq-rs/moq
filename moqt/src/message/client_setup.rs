use crate::message::{Role, Version};
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ClientSetup {
    pub supported_versions: Vec<Version>,
    pub role: Role,
    pub path: Option<String>,
}

impl Deserializer for ClientSetup {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let number_supported_versions = usize::deserialize(r)?;
        let mut supported_versions = Vec::with_capacity(number_supported_versions);
        for _ in 0..number_supported_versions {
            supported_versions.push(Version::deserialize(r)?);
        }

        let mut parameters = Parameters::deserialize(r)?;
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

impl Serializer for ClientSetup {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_versions.len().serialize(w)?;
        for supported_version in self.supported_versions.iter() {
            l += supported_version.serialize(w)?;
        }

        let mut parameters = Parameters::new();
        parameters.insert(ParameterKey::Role, self.role)?;
        if let Some(path) = self.path.as_ref() {
            parameters.insert(ParameterKey::Path, path.to_string())?;
        }
        l += parameters.serialize(w)?;

        Ok(l)
    }
}
