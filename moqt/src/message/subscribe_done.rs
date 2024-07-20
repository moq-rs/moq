use crate::message::message_parser::ErrorCode;
use crate::message::FullSequence;
use crate::{Deserializer, Error, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum SubscribeDoneCode {
    #[default]
    Unsubscribed = 0x0,
    InternalError = 0x1,
    Unauthorized = 0x2,
    TrackEnded = 0x3,
    SubscriptionEnded = 0x4,
    GoingAway = 0x5,
    Expired = 0x6,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeDone {
    pub subscribe_id: u64,

    pub status_code: u64,
    pub reason_phrase: String,

    pub final_group_object: Option<FullSequence>,
}

impl Deserializer for SubscribeDone {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;

        let (status_code, scl) = u64::deserialize(r)?;
        let (reason_phrase, rpl) = String::deserialize(r)?;

        let (exist, el) = bool::deserialize(r).map_err(|err| {
            if let Error::ErrInvalidBooleanValue(b) = err {
                Error::ErrParseError(
                    ErrorCode::ProtocolViolation,
                    format!("SUBSCRIBE_DONE ContentExists has invalid value {}", b),
                )
            } else {
                err
            }
        })?;
        let mut tl = sil + scl + rpl + el;
        let final_group_object = if exist {
            let (final_group_object, fgol) = FullSequence::deserialize(r)?;
            tl += fgol;
            Some(final_group_object)
        } else {
            None
        };

        Ok((
            Self {
                subscribe_id,

                status_code,
                reason_phrase,

                final_group_object,
            },
            tl,
        ))
    }
}

impl Serializer for SubscribeDone {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.status_code.serialize(w)?;
        l += self.reason_phrase.serialize(w)?;

        l += if let Some(group_object_pair) = self.final_group_object.as_ref() {
            true.serialize(w)? + group_object_pair.serialize(w)?
        } else {
            false.serialize(w)?
        };

        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_subscribe_done() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x0b, 0x02, 0x03, // subscribe_id = 2, error_code = 3,
            0x02, 0x68, 0x69, // reason_phrase = "hi"
            0x01, 0x08, 0x0c, // final_id = (8,12)
        ];

        let expected_message = ControlMessage::SubscribeDone(SubscribeDone {
            subscribe_id: 2,
            status_code: 3,
            reason_phrase: "hi".to_string(),
            final_group_object: Some(FullSequence {
                group_id: 8,
                object_id: 12,
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
