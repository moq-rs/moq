use crate::message::{Role, Version};
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ClientSetup {
    pub supported_versions: Vec<Version>,
    pub role: Role,
    pub path: Option<String>,
}

impl Deserializer for ClientSetup {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (number_supported_versions, mut tl) = usize::deserialize(r)?;
        let mut supported_versions = Vec::with_capacity(number_supported_versions);
        for _ in 0..number_supported_versions {
            let (version, vl) = Version::deserialize(r)?;
            supported_versions.push(version);
            tl += vl;
        }

        let (mut parameters, pl) = Parameters::deserialize(r)?;
        tl += pl;
        let role: Role = parameters
            .remove(ParameterKey::Role)
            .map_err(|err| Error::ErrProtocolViolation(err.to_string()))?
            .ok_or(Error::ErrProtocolViolation(
                "ROLE parameter missing".to_string(),
            ))?;
        let path: Option<String> = parameters.remove(ParameterKey::Path)?;

        Ok((
            Self {
                supported_versions,
                role,
                path,
            },
            tl,
        ))
    }
}

impl Serializer for ClientSetup {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_versions.len().serialize(w)?;
        for supported_version in self.supported_versions.iter() {
            l += supported_version.serialize(w)?;
        }

        let mut parameters = Parameters::new();
        parameters.insert(ParameterKey::Role, self.role)?;
        if let Some(path) = self.path.as_ref() {
            parameters.insert(ParameterKey::Path, path.to_string())?;
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
    fn test_client_setup() -> Result<()> {
        let tests: Vec<(Vec<u8>, ControlMessage)> = vec![
            (
                vec![
                    0x40, 0x40, // type
                    0x02, // versions
                    192, 0, 0, 0, 255, 0, 0, 1, // Draft01
                    192, 0, 0, 0, 255, 0, 0, 2,    // Draft02
                    0x02, // 2 parameters
                    0x00, 0x01, 0x03, // role = PubSub
                    0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
                ],
                ControlMessage::ClientSetup(ClientSetup {
                    supported_versions: vec![Version::Draft01, Version::Draft02],
                    role: Role::PubSub,
                    path: Some("foo".to_string()),
                }),
            ),
            (
                vec![
                    0x40, 0x40, 0x01, 0xc0, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00,
                    0x02, // 2 parameters
                    0x00, 0x01, 0x03, // role = PubSub
                    0x01, 0x01, 0x65, // path = "e"
                ],
                ControlMessage::ClientSetup(ClientSetup {
                    supported_versions: vec![Version::Draft00],
                    role: Role::PubSub,
                    path: Some("e".to_string()),
                }),
            ),
        ];

        for (expected_packet, expected_message) in tests {
            let mut cursor: Cursor<&[u8]> = Cursor::new(expected_packet.as_ref());
            let (actual_message, actual_len) = ControlMessage::deserialize(&mut cursor)?;
            assert_eq!(expected_message, actual_message);
            assert_eq!(expected_packet.len(), actual_len);

            let mut actual_packet = vec![];
            let _ = expected_message.serialize(&mut actual_packet)?;
            assert_eq!(expected_packet, actual_packet);
        }

        Ok(())
    }
}
