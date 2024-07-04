use crate::{Decodable, Encodable, Parameters, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Announce {
    pub track_namespace: String,
    pub parameters: Parameters,
}

impl Decodable for Announce {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::decode(r)?;
        let parameters = Parameters::decode(r)?;
        Ok(Self {
            track_namespace,
            parameters,
        })
    }
}

impl Encodable for Announce {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.encode(w)?;
        l += self.parameters.encode(w)?;
        Ok(l)
    }
}
