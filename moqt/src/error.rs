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
    #[error("duplicate parameter")]
    ErrDuplicateParameter,
    #[error("missing parameter")]
    ErrMissingParameter,
    #[error("unsupported parameter: {0}")]
    ErrUnsupportedParameter(u64),
    #[error("invalid message type: {0}")]
    ErrInvalidMessageType(u64),
    #[error("invalid filter type: {0}")]
    ErrInvalidFilterType(u64),
    #[error("invalid boolean value: {0}")]
    ErrInvalidBooleanValue(u8),
    #[error("unsupported version: {0}")]
    ErrUnsupportedVersion(u64),
    #[error("invalid role: {0}")]
    ErrInvalidRole(u64),
    #[error("track or group forward preference requires length")]
    ErrTrackGroupForwardPreferenceRequiresLength,
    #[error("object status must be kNormal if payload is non-empty")]
    ErrNonEmptyPayloadMustBeWithNormalObjectStatus,
    #[error("protocol violation: {0}")]
    ErrProtocolViolation(String),

    #[error("invalid string")]
    ErrInvalidString(#[from] FromUtf8Error),
}
