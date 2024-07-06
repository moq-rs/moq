use crate::{Error, Result};
use bytes::{Buf, BufMut, Bytes};

pub mod parameters;
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

impl Encodable for bool {
    /// Encode a varint to the given writer.
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        if !w.has_remaining_mut() {
            return Err(Error::ErrBufferTooShort);
        }
        w.put_u8(*self as u8);
        Ok(1)
    }
}

impl Decodable for bool {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        if !r.has_remaining() {
            return Err(Error::ErrBufferTooShort);
        }
        let b = r.get_u8();
        match b {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::ErrInvalidBooleanValue(b)),
        }
    }
}

impl Encodable for Bytes {
    /// Encode a varint to the given writer.
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        if !w.has_remaining_mut() {
            return Err(Error::ErrBufferTooShort);
        }
        w.put(self.slice(..));
        Ok(self.len())
    }
}

impl Decodable for Bytes {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        Ok(r.copy_to_bytes(r.remaining()))
    }
}

impl Decodable for String {
    fn decode<B: Buf>(r: &mut B) -> Result<Self> {
        let size = usize::decode(r)?;
        if r.remaining() < size {
            return Err(Error::ErrBufferTooShort);
        }

        let mut buf = vec![0; size];
        r.copy_to_slice(&mut buf);
        let str = String::from_utf8(buf)?;

        Ok(str)
    }
}

impl Encodable for String {
    fn encode<B: BufMut>(&self, w: &mut B) -> Result<usize> {
        let l = self.len().encode(w)?;
        if w.remaining_mut() < self.len() {
            return Err(Error::ErrBufferTooShort);
        }
        w.put(self.as_ref());
        Ok(l + self.len())
    }
}
