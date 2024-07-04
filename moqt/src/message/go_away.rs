use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct GoAway {
    pub new_session_uri: String,
}

impl Decodable for GoAway {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let new_session_uri = String::decode(r)?;
        Ok(Self { new_session_uri })
    }
}

impl Encodable for GoAway {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.new_session_uri.encode(w)
    }
}
