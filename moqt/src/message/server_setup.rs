use crate::message::message_parser::ParserErrorCode;
use crate::message::{Role, Version};
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ServerSetup {
    pub supported_version: Version,
    pub role: Option<Role>,
}

impl Deserializer for ServerSetup {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (supported_version, mut tl) = Version::deserialize(r)?;

        let (num_params, npl) = u64::deserialize(r)?;
        tl += npl;

        let mut role: Option<Role> = None;

        // Parse parameters
        for _ in 0..num_params {
            let (key, kl) = u64::deserialize(r)?;
            tl += kl;
            let (size, sl) = usize::deserialize(r)?;
            tl += sl;

            if r.remaining() < size {
                return Err(Error::ErrBufferTooShort);
            }

            if key == ParameterKey::Role as u64 {
                if role.is_some() {
                    return Err(Error::ErrParseError(
                        ParserErrorCode::ProtocolViolation,
                        "ROLE parameter appears twice in SETUP".to_string(),
                    ));
                }
                let (r, rl) = u64::deserialize(r)?;
                tl += rl;

                if rl != size {
                    return Err(Error::ErrParseError(
                        ParserErrorCode::ProtocolViolation,
                        "Parameter length does not match varint encoding".to_string(),
                    ));
                }

                role = Some(r.try_into().map_err(|_| {
                    Error::ErrParseError(
                        ParserErrorCode::ProtocolViolation,
                        "Invalid ROLE parameter".to_string(),
                    )
                })?);
            } else if key == ParameterKey::Path as u64 {
                return Err(Error::ErrParseError(
                    ParserErrorCode::ProtocolViolation,
                    "PATH parameter in SERVER_SETUP".to_string(),
                ));
            }
        }

        if role.is_none() {
            return Err(Error::ErrParseError(
                ParserErrorCode::ProtocolViolation,
                "ROLE parameter missing from SERVER_SETUP message".to_string(),
            ));
        }

        Ok((
            Self {
                supported_version,
                role,
            },
            tl,
        ))
    }
}

impl Serializer for ServerSetup {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_version.serialize(w)?;

        let mut parameters = Parameters::new();
        if let Some(role) = self.role.as_ref() {
            parameters.insert(ParameterKey::Role, *role)?;
        }
        l += parameters.serialize(w)?;
        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_server_setup() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x40, 0x41, // type
            192, 0, 0, 0, 255, 0, 0, 1,    // version Draft01
            0x01, // one param
            0x00, 0x01, 0x03, // role = PubSub
        ];

        let expected_message = ControlMessage::ServerSetup(ServerSetup {
            supported_version: Version::Draft01,
            role: Some(Role::PubSub),
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
