use crate::Result;
use bytes::{Buf, BufMut};

pub mod parameters;
pub mod string;
pub mod varint;

pub trait Decodable {
    fn decode<B>(r: &mut B) -> Result<Self>
    where
        Self: Sized,
        B: Buf;
}

pub trait Encodable {
    fn encode<B: BufMut>(&self, w: &mut B) -> Result<usize>;
}
