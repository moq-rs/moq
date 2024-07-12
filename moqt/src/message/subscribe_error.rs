use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum SubscribeErrorCode {
    #[default]
    InternalError = 0,
    InvalidRange = 1,
    RetryTrackAlias = 2,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeError {
    pub subscribe_id: u64,

    pub error_code: u64,
    pub reason_phrase: String,

    pub track_alias: u64,
}

impl Deserializer for SubscribeError {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;

        let (status_code, scl) = u64::deserialize(r)?;
        let (reason_phrase, rpl) = String::deserialize(r)?;

        let (track_alias, tal) = u64::deserialize(r)?;

        Ok((
            Self {
                subscribe_id,

                error_code: status_code,
                reason_phrase,

                track_alias,
            },
            sil + scl + rpl + tal,
        ))
    }
}

impl Serializer for SubscribeError {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.error_code.serialize(w)?;
        l += self.reason_phrase.serialize(w)?;

        l += self.track_alias.serialize(w)?;

        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_subscribe_error() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x05, 0x02, // subscribe_id = 2
            0x01, // error_code = 1
            0x03, 0x62, 0x61, 0x72, // reason_phrase = "bar"
            0x04, // track_alias = 4,
        ];

        let expected_message = ControlMessage::SubscribeError(SubscribeError {
            subscribe_id: 2,
            error_code: SubscribeErrorCode::InvalidRange as u64,
            reason_phrase: "bar".to_string(),
            track_alias: 4,
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
