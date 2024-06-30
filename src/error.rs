use std::string::FromUtf8Error;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    #[error("value too large for varint encoding")]
    ErrVarIntBoundsExceeded,
    #[error("unexpected buffer end")]
    ErrUnexpectedEnd,
    #[error("malformed varint")]
    ErrMalformedVarInt,
    #[error("buffer is too short")]
    ErrBufferTooShort,

    #[error("invalid string")]
    InvalidString(#[from] FromUtf8Error),
}
