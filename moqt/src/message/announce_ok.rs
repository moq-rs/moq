use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceOk {
    pub track_namespace: String,
}

impl Deserializer for AnnounceOk {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        Ok((Self { track_namespace }, tnsl))
    }
}

impl Serializer for AnnounceOk {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.track_namespace.serialize(w)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::Message;
    use std::io::Cursor;

    #[test]
    fn test_announce_ok() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x07, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
        ];

        let expected_message = Message::AnnounceOk(AnnounceOk {
            track_namespace: "foo".to_string(),
        });

        let mut cursor: Cursor<&[u8]> = Cursor::new(expected_packet.as_ref());
        let (actual_message, actual_len) = Message::deserialize(&mut cursor)?;
        assert_eq!(expected_message, actual_message);
        assert_eq!(expected_packet.len(), actual_len);

        let mut actual_packet = vec![];
        let _ = expected_message.serialize(&mut actual_packet)?;
        assert_eq!(expected_packet, actual_packet);

        Ok(())
    }
}
