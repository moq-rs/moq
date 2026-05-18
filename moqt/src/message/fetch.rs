use crate::message::message_parser::ErrorCode;
use crate::message::{FullSequence, FullTrackName};
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FetchType {
    Standalone = 0x1,
    RelativeJoining = 0x2,
    AbsoluteJoining = 0x3,
}

impl TryFrom<u64> for FetchType {
    type Error = Error;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        match value {
            0x1 => Ok(Self::Standalone),
            0x2 => Ok(Self::RelativeJoining),
            0x3 => Ok(Self::AbsoluteJoining),
            _ => Err(Error::ErrParseError(
                ErrorCode::ProtocolViolation,
                format!("Invalid FETCH type {}", value),
            )),
        }
    }
}

impl Deserializer for FetchType {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (value, len) = u64::deserialize(r)?;
        Ok((value.try_into()?, len))
    }
}

impl Serializer for FetchType {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        (*self as u64).serialize(w)
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct StandaloneFetch {
    pub full_track_name: FullTrackName,
    pub start: FullSequence,
    pub end: FullSequence,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct JoiningFetch {
    pub joining_request_id: u64,
    pub joining_start: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FetchTarget {
    Standalone(StandaloneFetch),
    RelativeJoining(JoiningFetch),
    AbsoluteJoining(JoiningFetch),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Fetch {
    pub request_id: u64,
    pub target: FetchTarget,
    pub authorization_info: Option<String>,
}

impl Fetch {
    fn validate_range(start: FullSequence, end: FullSequence) -> Result<()> {
        if end.group_id < start.group_id {
            return Err(Error::ErrParseError(
                ErrorCode::ProtocolViolation,
                "End group is less than start group in FETCH".to_string(),
            ));
        }
        if end.group_id == start.group_id && end.object_id < start.object_id {
            return Err(Error::ErrParseError(
                ErrorCode::ProtocolViolation,
                "End object comes before start object in FETCH".to_string(),
            ));
        }
        Ok(())
    }
}

impl Deserializer for Fetch {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (request_id, request_len) = u64::deserialize(r)?;
        let (fetch_type, type_len) = FetchType::deserialize(r)?;

        let (target, body_len) = match fetch_type {
            FetchType::Standalone => {
                let (full_track_name, name_len) = FullTrackName::deserialize(r)?;
                let (start, start_len) = FullSequence::deserialize(r)?;
                let (mut end, end_len) = FullSequence::deserialize(r)?;
                if end.object_id == 0 {
                    end.object_id = u64::MAX;
                } else {
                    end.object_id -= 1;
                }
                Self::validate_range(start, end)?;
                (
                    FetchTarget::Standalone(StandaloneFetch {
                        full_track_name,
                        start,
                        end,
                    }),
                    name_len + start_len + end_len,
                )
            }
            FetchType::RelativeJoining => {
                let (joining_request_id, request_id_len) = u64::deserialize(r)?;
                let (joining_start, joining_start_len) = u64::deserialize(r)?;
                (
                    FetchTarget::RelativeJoining(JoiningFetch {
                        joining_request_id,
                        joining_start,
                    }),
                    request_id_len + joining_start_len,
                )
            }
            FetchType::AbsoluteJoining => {
                let (joining_request_id, request_id_len) = u64::deserialize(r)?;
                let (joining_start, joining_start_len) = u64::deserialize(r)?;
                (
                    FetchTarget::AbsoluteJoining(JoiningFetch {
                        joining_request_id,
                        joining_start,
                    }),
                    request_id_len + joining_start_len,
                )
            }
        };

        let (num_params, mut param_len) = u64::deserialize(r)?;
        let mut authorization_info = None;
        for _ in 0..num_params {
            let (key, key_len) = u64::deserialize(r)?;
            param_len += key_len;
            let (size, size_len) = usize::deserialize(r)?;
            param_len += size_len;
            if r.remaining() < size {
                return Err(Error::ErrBufferTooShort);
            }
            if key == ParameterKey::AuthorizationInfo as u64 {
                if authorization_info.is_some() {
                    return Err(Error::ErrParseError(
                        ErrorCode::ProtocolViolation,
                        "AUTHORIZATION_INFO parameter appears twice in FETCH".to_string(),
                    ));
                }
                let mut buf = vec![0; size];
                r.copy_to_slice(&mut buf);
                param_len += size;
                authorization_info = Some(String::from_utf8(buf)?);
            } else {
                r.advance(size);
                param_len += size;
            }
        }

        Ok((
            Self {
                request_id,
                target,
                authorization_info,
            },
            request_len + type_len + body_len + param_len,
        ))
    }
}

impl Serializer for Fetch {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut len = self.request_id.serialize(w)?;
        match &self.target {
            FetchTarget::Standalone(fetch) => {
                Self::validate_range(fetch.start, fetch.end)?;
                len += FetchType::Standalone.serialize(w)?;
                len += fetch.full_track_name.serialize(w)?;
                len += fetch.start.serialize(w)?;
                let mut end = fetch.end;
                if end.object_id == u64::MAX {
                    end.object_id = 0;
                } else {
                    end.object_id += 1;
                }
                len += end.serialize(w)?;
            }
            FetchTarget::RelativeJoining(fetch) => {
                len += FetchType::RelativeJoining.serialize(w)?;
                len += fetch.joining_request_id.serialize(w)?;
                len += fetch.joining_start.serialize(w)?;
            }
            FetchTarget::AbsoluteJoining(fetch) => {
                len += FetchType::AbsoluteJoining.serialize(w)?;
                len += fetch.joining_request_id.serialize(w)?;
                len += fetch.joining_start.serialize(w)?;
            }
        }

        if let Some(authorization_info) = self.authorization_info.as_ref() {
            let mut parameters = Parameters::new();
            parameters.insert(
                ParameterKey::AuthorizationInfo,
                authorization_info.to_string(),
            )?;
            len += parameters.serialize(w)?;
        } else {
            len += 0u64.serialize(w)?;
        }

        Ok(len)
    }
}
