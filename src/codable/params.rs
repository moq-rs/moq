use crate::codable::{Decodable, Encodable};
use crate::{Error, Result};
use bytes::{Buf, BufMut};
use std::collections::HashMap;
use std::io::Cursor;

#[derive(Default, Debug, Clone)]
pub struct Params(pub HashMap<u64, Vec<u8>>);

impl Decodable for Params {
    fn decode<R: Buf>(mut r: &mut R) -> Result<Self> {
        let mut params = HashMap::new();

        // I hate this encoding so much; let me encode my role and get on with my life.
        let count = u64::decode(r)?;
        for _ in 0..count {
            let kind = u64::decode(r)?;
            if params.contains_key(&kind) {
                return Err(Error::ErrDupliateParameter);
            }

            let size = usize::decode(&mut r)?;
            if r.remaining() < size {
                return Err(Error::ErrBufferTooShort);
            }

            // Don't allocate the entire requested size to avoid a possible attack
            // Instead, we allocate up to 1024 and keep appending as we read further.
            let mut buf = vec![0; size];
            r.copy_to_slice(&mut buf);

            params.insert(kind, buf);
        }

        Ok(Params(params))
    }
}

impl Encodable for Params {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.0.len().encode(w)?;

        for (kind, value) in self.0.iter() {
            l += kind.encode(w)?;
            l += value.len().encode(w)?;
            if w.remaining_mut() < value.len() {
                return Err(Error::ErrBufferTooShort);
            }
            w.put_slice(value);
            l += value.len();
        }

        Ok(l)
    }
}

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<P: Encodable>(&mut self, kind: u64, p: P) -> Result<()> {
        if self.contains(kind) {
            return Err(Error::ErrDupliateParameter);
        }
        let mut value = Vec::new();
        p.encode(&mut value)?;
        self.0.insert(kind, value);
        Ok(())
    }

    pub fn contains(&self, kind: u64) -> bool {
        self.0.contains_key(&kind)
    }

    pub fn remove<P: Decodable>(&mut self, kind: u64) -> Option<P> {
        if let Some(value) = self.0.remove(&kind) {
            let mut cursor = Cursor::new(value);
            P::decode(&mut cursor).ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::codable::varint::VarInt;

    #[test]
    fn test_params() -> Result<()> {
        let mut params = Params::new();

        params.insert(1, "I am string".to_string())?;
        params.insert(2, 100u64)?;
        params.insert(3, 101usize)?;
        params.insert(4, VarInt::from_u64(2u64.pow(5))?)?;
        params.insert(5, VarInt::from_u64(2u64.pow(13))?)?;
        params.insert(6, VarInt::from_u64(2u64.pow(28))?)?;
        params.insert(7, VarInt::from_u64(2u64.pow(61))?)?;

        let result = params.insert(1, "I am another string".to_string());
        assert!(result.is_err());

        assert!(params.contains(1));
        assert!(params.contains(2));
        assert!(!params.contains(10));

        assert_eq!(Some("I am string".to_string()), params.remove(1));
        assert_eq!(Some(100u64), params.remove(2));
        assert_eq!(Some(101usize), params.remove(3));
        assert_eq!(Some(2u64.pow(5)), params.remove(4));
        assert_eq!(Some(2u64.pow(13)), params.remove(5));
        assert_eq!(Some(2u64.pow(28)), params.remove(6));
        assert_eq!(Some(2u64.pow(61)), params.remove(7));

        Ok(())
    }
}
