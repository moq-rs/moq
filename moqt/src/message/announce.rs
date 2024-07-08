use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Announce {
    pub track_namespace: String,
    pub authorization_info: Option<String>,
}

impl Deserializer for Announce {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;

        let (mut parameters, pl) = Parameters::deserialize(r)?;
        let authorization_info: Option<String> = parameters.remove(ParameterKey::AuthorizationInfo);

        Ok((
            Self {
                track_namespace,
                authorization_info,
            },
            tnsl + pl,
        ))
    }
}

impl Serializer for Announce {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.track_namespace.serialize(w)?;

        if let Some(authorization_info) = self.authorization_info.as_ref() {
            let mut parameters = Parameters::new();
            parameters.insert(
                ParameterKey::AuthorizationInfo,
                authorization_info.to_string(),
            )?;
            l += parameters.serialize(w)?;
        }

        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::Message;
    use std::io::Cursor;

    #[test]
    fn test_announce() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x06, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x01, // 1 parameter
            0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        ];

        let expected_message = Message::Announce(Announce {
            track_namespace: "foo".to_string(),
            authorization_info: Some("bar".to_string()),
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
