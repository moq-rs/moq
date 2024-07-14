use crate::message::FullSequence;
use crate::serde::parameters::ParameterKey;
use crate::Result;
use crate::{Deserializer, Parameters, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeUpdate {
    pub subscribe_id: u64,

    pub start_group_object: FullSequence,
    pub end_group_object: Option<FullSequence>,

    pub authorization_info: Option<String>,
}

impl Deserializer for SubscribeUpdate {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (subscribe_id, sil) = u64::deserialize(r)?;

        let (start_group_object, sgol) = FullSequence::deserialize(r)?;
        let (end_group_object, egol) = FullSequence::deserialize(r)?;

        let (mut parameters, pl) = Parameters::deserialize(r)?;
        let authorization_info: Option<String> =
            parameters.remove(ParameterKey::AuthorizationInfo)?;

        Ok((
            Self {
                subscribe_id,

                start_group_object,
                end_group_object: if end_group_object.group_id == 0 {
                    None
                } else if end_group_object.object_id == 0 {
                    Some(FullSequence {
                        group_id: end_group_object.group_id - 1,
                        object_id: u64::MAX,
                    })
                } else {
                    Some(FullSequence {
                        group_id: end_group_object.group_id - 1,
                        object_id: end_group_object.object_id - 1,
                    })
                },

                authorization_info,
            },
            sil + sgol + egol + pl,
        ))
    }
}

impl Serializer for SubscribeUpdate {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.serialize(w)?;

        l += self.start_group_object.serialize(w)?;
        if let Some(end_group_object) = self.end_group_object.as_ref() {
            if end_group_object.object_id == u64::MAX {
                l += FullSequence {
                    group_id: end_group_object.group_id + 1,
                    object_id: 0,
                }
                .serialize(w)?;
            } else {
                l += FullSequence {
                    group_id: end_group_object.group_id + 1,
                    object_id: end_group_object.object_id + 1,
                }
                .serialize(w)?;
            }
        } else {
            l += FullSequence {
                group_id: 0,
                object_id: 0,
            }
            .serialize(w)?;
        }

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
    fn test_subscribe_update() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x02, 0x02, 0x03, 0x01, 0x05, 0x06, // start and end sequences
            0x01, // 1 parameter
            0x02, 0x03, 0x62, 0x61, 0x72, // authorization_info = "bar"
        ];

        let expected_message = ControlMessage::SubscribeUpdate(SubscribeUpdate {
            subscribe_id: 2,
            start_group_object: FullSequence {
                group_id: 3,
                object_id: 1,
            },
            end_group_object: Some(FullSequence {
                group_id: 4,
                object_id: 5,
            }),
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
