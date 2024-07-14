use crate::message::message_parser::ParserErrorCode;
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Announce {
    pub track_namespace: String,
    pub authorization_info: Option<String>,
}

impl Deserializer for Announce {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;

        let mut authorization_info: Option<String> = None;
        let (num_params, mut pl) = u64::deserialize(r)?;
        // Parse parameters
        for _ in 0..num_params {
            let (key, kl) = u64::deserialize(r)?;
            pl += kl;
            let (size, sl) = usize::deserialize(r)?;
            pl += sl;

            if r.remaining() < size {
                return Err(Error::ErrBufferTooShort);
            }

            if key == ParameterKey::AuthorizationInfo as u64 {
                if authorization_info.is_some() {
                    return Err(Error::ErrParseError(
                        ParserErrorCode::ProtocolViolation,
                        "AUTHORIZATION_INFO parameter appears twice in ANNOUNCE".to_string(),
                    ));
                }
                let mut buf = vec![0; size];
                r.copy_to_slice(&mut buf);
                pl += size;

                authorization_info = Some(String::from_utf8(buf)?);
            }
        }

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
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_announce() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x06, 0x03, 0x66, 0x6f, 0x6f, // track_namespace = "foo"
            0x01, // 1 parameter
            0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        ];

        let expected_message = ControlMessage::Announce(Announce {
            track_namespace: "foo".to_string(),
            authorization_info: Some("bar".to_string()),
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
