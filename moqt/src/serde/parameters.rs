use crate::serde::{Deserializer, Serializer};
use crate::{Error, Result};
use bytes::{Buf, BufMut};
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

impl Deserializer for Parameters {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let mut params = HashMap::new();

        // I hate this encoding so much; let me encode my role and get on with my life.
        let (count, mut tl) = u64::deserialize(r)?;
        for _ in 0..count {
            let (kind, kl) = u64::deserialize(r)?;
            if params.contains_key(&kind) {
                return Err(Error::ErrDuplicateParameter);
            }

            let (size, sl) = usize::deserialize(r)?;
            if r.remaining() < size {
                return Err(Error::ErrBufferTooShort);
            }

            // Don't allocate the entire requested size to avoid a possible attack
            // Instead, we allocate up to 1024 and keep appending as we read further.
            let mut buf = vec![0; size];
            r.copy_to_slice(&mut buf);

            params.insert(kind, buf);
            tl += kl + sl + size;
        }

        Ok((Parameters(params), tl))
    }
}

impl Serializer for Parameters {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.0.len().serialize(w)?;

        for (kind, value) in self.0.iter() {
            l += kind.serialize(w)?;
            l += value.len().serialize(w)?;
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

    pub fn remove<P: Deserializer>(&mut self, key: ParameterKey) -> Option<P> {
        if let Some(value) = self.0.remove(&(key as u64)) {
            let mut cursor = Cursor::new(value);
            P::deserialize(&mut cursor).ok().map(|v| v.0)
        } else {
            None
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

        assert_eq!(Some(Role::PubSub), params.remove(ParameterKey::Role));
        assert_eq!(
            Some("/moq/1".to_string()),
            params.remove(ParameterKey::Path)
        );
        assert_eq!(
            Some("password".to_string()),
            params.remove(ParameterKey::AuthorizationInfo)
        );
        Ok(())
    }
}
