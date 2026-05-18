use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct FetchOk {
    pub request_id: u64,
    pub end_of_track: bool,
    pub end_location: crate::message::FullSequence,
}

impl Deserializer for FetchOk {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (request_id, request_len) = u64::deserialize(r)?;
        let (end_of_track, end_track_len) = bool::deserialize(r)?;
        let (mut end_location, end_len) = crate::message::FullSequence::deserialize(r)?;
        if end_location.object_id == 0 {
            end_location.object_id = u64::MAX;
        } else {
            end_location.object_id -= 1;
        }
        let (num_params, params_len) = u64::deserialize(r)?;
        let mut trailing = 0;
        for _ in 0..num_params {
            let (_key, key_len) = u64::deserialize(r)?;
            let (size, size_len) = usize::deserialize(r)?;
            if r.remaining() < size {
                return Err(crate::Error::ErrBufferTooShort);
            }
            r.advance(size);
            trailing += key_len + size_len + size;
        }
        Ok((
            Self {
                request_id,
                end_of_track,
                end_location,
            },
            request_len + end_track_len + end_len + params_len + trailing,
        ))
    }
}

impl Serializer for FetchOk {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut len = self.request_id.serialize(w)?;
        len += self.end_of_track.serialize(w)?;
        let mut end_location = self.end_location;
        if end_location.object_id == u64::MAX {
            end_location.object_id = 0;
        } else {
            end_location.object_id += 1;
        }
        len += end_location.serialize(w)?;
        len += 0u64.serialize(w)?;
        Ok(len)
    }
}
