use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnSubscribe {
    pub subscribe_id: u64,
}

impl Decodable for UnSubscribe {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::decode(r)?;
        Ok(Self { subscribe_id })
    }
}

impl Encodable for UnSubscribe {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.subscribe_id.encode(w)
    }
}
