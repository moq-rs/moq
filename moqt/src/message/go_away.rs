use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct GoAway {
    pub new_session_uri: String,
}

impl Deserializer for GoAway {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (new_session_uri, nsul) = String::deserialize(r)?;
        Ok((Self { new_session_uri }, nsul))
    }
}

impl Serializer for GoAway {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.new_session_uri.serialize(w)
    }
}
