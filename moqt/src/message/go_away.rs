use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct GoAway {
    pub new_session_uri: String,
}

impl Deserializer for GoAway {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (new_session_uri, nsul) = String::deserialize(r)?;
        Ok((Self { new_session_uri }, nsul))
    }
}

impl Serializer for GoAway {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.new_session_uri.serialize(w)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::Message;
    use std::io::Cursor;

    #[test]
    fn test_go_away() -> Result<()> {
        let expected_packet: Vec<u8> = vec![0x10, 0x03, 0x66, 0x6f, 0x6f];

        let expected_message = Message::GoAway(GoAway {
            new_session_uri: "foo".to_string(),
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
