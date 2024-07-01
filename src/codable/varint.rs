use crate::codable::{Decodable, Encodable};
use crate::{Error, Result};
use bytes::{Buf, BufMut};
use std::fmt;

/// An integer less than 2^62
///
/// Values of this type are suitable for encoding as QUIC variable-length integer.
/// It would be neat if we could express to Rust that the top two bits are available for use as enum
/// discriminants
#[derive(Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VarInt(pub(crate) u64);

impl VarInt {
    /// The largest representable value
    pub const MAX: Self = Self((1 << 62) - 1);
    /// The largest encoded value length
    pub const MAX_SIZE: usize = 8;

    /// Construct a `VarInt` infallibly
    pub const fn from_u32(x: u32) -> Self {
        Self(x as u64)
    }

    /// Succeeds iff `x` < 2^62
    pub fn from_u64(x: u64) -> Result<Self> {
        if x < 2u64.pow(62) {
            Ok(Self(x))
        } else {
            Err(Error::ErrVarIntBoundsExceeded)
        }
    }

    /// Create a VarInt without ensuring it's in range
    ///
    /// # Safety
    ///
    /// `x` must be less than 2^62.
    pub const unsafe fn from_u64_unchecked(x: u64) -> Self {
        Self(x)
    }

    /// Extract the integer value
    pub const fn into_inner(self) -> u64 {
        self.0
    }

    /// Compute the number of bytes needed to encode this value
    pub(crate) fn size(self) -> usize {
        let x = self.0;
        if x < 2u64.pow(6) {
            1
        } else if x < 2u64.pow(14) {
            2
        } else if x < 2u64.pow(30) {
            4
        } else if x < 2u64.pow(62) {
            8
        } else {
            unreachable!("malformed VarInt");
        }
    }
}

impl From<VarInt> for u64 {
    fn from(x: VarInt) -> Self {
        x.0
    }
}

impl From<u8> for VarInt {
    fn from(x: u8) -> Self {
        Self(x.into())
    }
}

impl From<u16> for VarInt {
    fn from(x: u16) -> Self {
        Self(x.into())
    }
}

impl From<u32> for VarInt {
    fn from(x: u32) -> Self {
        Self(x.into())
    }
}

impl std::convert::TryFrom<u64> for VarInt {
    type Error = Error;
    /// Succeeds iff `x` < 2^62
    fn try_from(x: u64) -> std::result::Result<Self, Self::Error> {
        Self::from_u64(x)
    }
}

impl std::convert::TryFrom<u128> for VarInt {
    type Error = Error;
    /// Succeeds iff `x` < 2^62
    fn try_from(x: u128) -> std::result::Result<Self, Self::Error> {
        Self::from_u64(x.try_into().map_err(|_| Error::ErrVarIntBoundsExceeded)?)
    }
}

impl std::convert::TryFrom<usize> for VarInt {
    type Error = Error;
    /// Succeeds iff `x` < 2^62
    fn try_from(x: usize) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x as u64)
    }
}

impl fmt::Debug for VarInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for VarInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Decodable for VarInt {
    fn decode<B: Buf>(r: &mut B) -> Result<Self> {
        if !r.has_remaining() {
            return Err(Error::ErrUnexpectedEnd);
        }
        let mut buf = [0; 8];
        buf[0] = r.get_u8();
        let tag = buf[0] >> 6;
        buf[0] &= 0b0011_1111;
        let x = match tag {
            0b00 => u64::from(buf[0]),
            0b01 => {
                if r.remaining() < 1 {
                    return Err(Error::ErrUnexpectedEnd);
                }
                r.copy_to_slice(&mut buf[1..2]);
                u64::from(u16::from_be_bytes(buf[..2].try_into().unwrap()))
            }
            0b10 => {
                if r.remaining() < 3 {
                    return Err(Error::ErrUnexpectedEnd);
                }
                r.copy_to_slice(&mut buf[1..4]);
                u64::from(u32::from_be_bytes(buf[..4].try_into().unwrap()))
            }
            0b11 => {
                if r.remaining() < 7 {
                    return Err(Error::ErrUnexpectedEnd);
                }
                r.copy_to_slice(&mut buf[1..8]);
                u64::from_be_bytes(buf)
            }
            _ => unreachable!(),
        };
        Ok(Self(x))
    }
}

impl Encodable for VarInt {
    fn encode<B: BufMut>(&self, w: &mut B) -> Result<usize> {
        let x = self.0;
        if x < 2u64.pow(6) {
            if w.remaining_mut() < 1 {
                return Err(Error::ErrBufferTooShort);
            }
            w.put_u8(x as u8);
            Ok(1)
        } else if x < 2u64.pow(14) {
            if w.remaining_mut() < 2 {
                return Err(Error::ErrBufferTooShort);
            }
            w.put_u16(0b01 << 14 | x as u16);
            Ok(2)
        } else if x < 2u64.pow(30) {
            if w.remaining_mut() < 4 {
                return Err(Error::ErrBufferTooShort);
            }
            w.put_u32(0b10 << 30 | x as u32);
            Ok(4)
        } else if x < 2u64.pow(62) {
            if w.remaining_mut() < 8 {
                return Err(Error::ErrBufferTooShort);
            }
            w.put_u64(0b11 << 62 | x);
            Ok(8)
        } else {
            Err(Error::ErrMalformedVarInt)
        }
    }
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

impl Encodable for u64 {
    /// Encode a varint to the given writer.
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let var = VarInt::try_from(*self)?;
        var.encode(w)
    }
}

impl Decodable for u64 {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        VarInt::decode(r).map(|v| v.into_inner())
    }
}

impl Encodable for usize {
    /// Encode a varint to the given writer.
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let var = VarInt::try_from(*self)?;
        var.encode(w)
    }
}

impl Decodable for usize {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        VarInt::decode(r).map(|v| v.into_inner() as usize)
    }
}
