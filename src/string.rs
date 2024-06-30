use crate::codable::{Decodable, Encodable};
use crate::{Error, Result};
use bytes::{Buf, BufMut};

impl Decodable for String {
    fn decode<B: Buf>(r: &mut B) -> Result<Self> {
        let len = usize::decode(r)?;
        if r.remaining() < len {
            return Err(Error::ErrBufferTooShort);
        }

        let mut buf = vec![0; len];
        r.copy_to_slice(&mut buf);
        let str = String::from_utf8(buf)?;

        Ok(str)
    }
}

impl Encodable for String {
    fn encode<B: BufMut>(&self, w: &mut B) -> Result<usize> {
        let len = self.len().encode(w)?;
        if w.remaining_mut() < self.len() {
            return Err(Error::ErrBufferTooShort);
        }
        w.put(self.as_ref());
        Ok(len + self.len())
    }
}
