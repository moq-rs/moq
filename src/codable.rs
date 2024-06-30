use crate::Result;
use bytes::{Buf, BufMut};

pub trait Decodable {
    fn decode<B>(buf: &mut B) -> Result<Self>
    where
        Self: Sized,
        B: Buf;
}

pub trait Encodable {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<usize>;
}
