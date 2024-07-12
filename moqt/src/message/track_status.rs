use crate::message::FullSequence;
use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum TrackStatusCode {
    #[default]
    InProgress = 0x0,
    DoesNotExist = 0x1,
    NotYetBegun = 0x2,
    Finished = 0x3,
    StatusNotAvailable = 0x4,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct TrackStatus {
    pub track_namespace: String,
    pub track_name: String,
    pub status_code: u64,
    pub last_group_object: FullSequence,
}

impl Deserializer for TrackStatus {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        let (track_name, tnl) = String::deserialize(r)?;
        let (status_code, scl) = u64::deserialize(r)?;
        let (last_group_object, lgol) = FullSequence::deserialize(r)?;
        Ok((
            Self {
                track_namespace,
                track_name,
                status_code,
                last_group_object,
            },
            tnsl + tnl + scl + lgol,
        ))
    }
}

impl Serializer for TrackStatus {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;
        l += self.track_name.serialize(w)?;
        l += self.status_code.serialize(w)?;
        l += self.last_group_object.serialize(w)?;
        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_track_status() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x0e, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x04, 0x61, 0x62, 0x63, 0x64, // track_name = "abcd"
            0x00, 0x0c, 0x14, // status, last_group, last_object
        ];

        let expected_message = ControlMessage::TrackStatus(TrackStatus {
            track_namespace: "foo".to_string(),
            track_name: "abcd".to_string(),
            status_code: TrackStatusCode::InProgress as u64,
            last_group_object: FullSequence {
                group_id: 12,
                object_id: 20,
            },
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
