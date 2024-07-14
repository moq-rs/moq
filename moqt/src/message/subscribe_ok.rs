use crate::message::message_parser::ParserErrorCode;
use crate::message::FullSequence;
use crate::{Deserializer, Error, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeOk {
    pub subscribe_id: u64,

    pub expires: u64,

    pub largest_group_object: Option<FullSequence>,
}

impl Deserializer for SubscribeOk {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;

        let (expires, el) = u64::deserialize(r)?;

        let (exist, l) = bool::deserialize(r).map_err(|err| {
            if let Error::ErrInvalidBooleanValue(b) = err {
                Error::ErrParseError(
                    ParserErrorCode::ProtocolViolation,
                    format!("SUBSCRIBE_OK ContentExists has invalid value {}", b),
                )
            } else {
                err
            }
        })?;
        let mut tl = sil + el + l;
        let largest_group_object = if exist {
            let (largest_group_object, lgol) = FullSequence::deserialize(r)?;
            tl += lgol;
            Some(largest_group_object)
        } else {
            None
        };

        Ok((
            Self {
                subscribe_id,

                expires,

                largest_group_object,
            },
            tl,
        ))
    }
}

impl Serializer for SubscribeOk {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.expires.serialize(w)?;

        l += if let Some(largest_group_object) = self.largest_group_object.as_ref() {
            true.serialize(w)? + largest_group_object.serialize(w)?
        } else {
            false.serialize(w)?
        };

        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::{ControlMessage, FullSequence};
    use std::io::Cursor;

    #[test]
    fn test_subscribe_ok() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x04, 0x01, 0x03, // subscribe_id = 1, expires = 3
            0x01, 0x0c, 0x14, // largest_group_id = 12, largest_object_id = 20,
        ];

        let expected_message = ControlMessage::SubscribeOk(SubscribeOk {
            subscribe_id: 1,
            expires: 3,
            largest_group_object: Some(FullSequence {
                group_id: 12,
                object_id: 20,
            }),
        });

        let mut cursor: Cursor<&[u8]> = Cursor::new(expected_packet.as_ref());
        let (actual_message, actual_len) = ControlMessage::deserialize(&mut cursor)?;
        assert_eq!(expected_message, actual_message);
        assert_eq!(expected_packet.len(), actual_len);

        let mut actual_packet = vec![];
        let _ = expected_message.serialize(&mut actual_packet)?;
        assert_eq!(expected_packet, actual_packet);

        Ok(())
    }
}
