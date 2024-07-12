use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum AnnounceErrorCode {
    #[default]
    InternalError = 0,
    AnnounceNotSupported = 1,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceErrorReason {
    pub error_code: AnnounceErrorCode,
    pub reason_phrase: String,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceError {
    pub track_namespace: String,
    pub error_code: u64,
    pub reason_phrase: String,
}

impl Deserializer for AnnounceError {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        let (error_code, ecl) = u64::deserialize(r)?;
        let (reason_phrase, rpl) = String::deserialize(r)?;
        Ok((
            Self {
                track_namespace,
                error_code,
                reason_phrase,
            },
            tnsl + ecl + rpl,
        ))
    }
}

impl Serializer for AnnounceError {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;
        l += self.error_code.serialize(w)?;
        l += self.reason_phrase.serialize(w)?;
        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_announce_error() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x08, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x01, // error_code = 1
            0x03, 0x62, 0x61, 0x72, // reason_phrase = "bar"
        ];

        let expected_message = ControlMessage::AnnounceError(AnnounceError {
            track_namespace: "foo".to_string(),
            error_code: 1,
            reason_phrase: "bar".to_string(),
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
