use crate::serde::{Deserializer, Serializer};
use crate::{Error, Result};
use bytes::BufMut;
use std::collections::HashMap;
use std::io::Cursor;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParameterKey {
    #[default]
    Role = 0,
    Path = 1,
    AuthorizationInfo = 2,
}

impl TryFrom<u64> for ParameterKey {
    type Error = Error;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        match value {
            0x1 => Ok(ParameterKey::Role),
            0x2 => Ok(ParameterKey::Path),
            0x3 => Ok(ParameterKey::AuthorizationInfo),
            _ => Err(Error::ErrUnsupportedParameter(value)),
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Parameters(pub HashMap<u64, Vec<u8>>);

impl Serializer for Parameters {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.0.len().serialize(w)?;

        #[allow(clippy::map_clone)]
        let mut kinds: Vec<u64> = self.0.keys().map(|key| *key).collect();
        kinds.sort();
        for kind in kinds {
            l += kind.serialize(w)?;
            let value = &self.0[&kind];
            if !(kind == ParameterKey::Path as u64
                || kind == ParameterKey::AuthorizationInfo as u64)
            {
                l += value.len().serialize(w)?;
            }
            if w.remaining_mut() < value.len() {
                return Err(Error::ErrBufferTooShort);
            }
            w.put_slice(value);
            l += value.len();
        }

        Ok(l)
    }
}

impl Parameters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<P: Serializer>(&mut self, key: ParameterKey, p: P) -> Result<()> {
        if self.contains(key) {
            return Err(Error::ErrDuplicateParameter);
        }
        let mut value = Vec::new();
        p.serialize(&mut value)?;
        self.0.insert(key as u64, value);
        Ok(())
    }

    pub fn contains(&self, key: ParameterKey) -> bool {
        self.0.contains_key(&(key as u64))
    }

    pub fn remove<P: Deserializer>(&mut self, key: ParameterKey) -> Result<Option<P>> {
        if let Some(value) = self.0.remove(&(key as u64)) {
            let mut cursor = Cursor::new(value);
            let (p, _) = P::deserialize(&mut cursor)?;
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::Role;

    #[test]
    fn test_params() -> Result<()> {
        let mut params = Parameters::new();

        params.insert(ParameterKey::Role, Role::PubSub)?;
        params.insert(ParameterKey::Path, "/moq/1".to_string())?;

        assert!(!params.contains(ParameterKey::AuthorizationInfo));
        params.insert(ParameterKey::AuthorizationInfo, "password".to_string())?;

        let result = params.insert(ParameterKey::Path, "/moq/2".to_string());
        assert!(result.is_err());

        assert!(params.contains(ParameterKey::Role));
        assert!(params.contains(ParameterKey::Path));
        assert!(params.contains(ParameterKey::AuthorizationInfo));

        assert_eq!(Some(Role::PubSub), params.remove(ParameterKey::Role)?);
        assert_eq!(
            Some("/moq/1".to_string()),
            params.remove(ParameterKey::Path)?
        );
        assert_eq!(
            Some("password".to_string()),
            params.remove(ParameterKey::AuthorizationInfo)?
        );
        Ok(())
    }
}
