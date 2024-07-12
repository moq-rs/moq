use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnSubscribe {
    pub subscribe_id: u64,
}

impl Deserializer for UnSubscribe {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;
        Ok((Self { subscribe_id }, sil))
    }
}

impl Serializer for UnSubscribe {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.subscribe_id.serialize(w)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_unsubscribe() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x0a, 0x03, // subscribe_id = 3
        ];

        let expected_message = ControlMessage::UnSubscribe(UnSubscribe { subscribe_id: 3 });

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
