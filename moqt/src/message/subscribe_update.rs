use crate::message::message_parser::ParserErrorCode;
use crate::message::FullSequence;
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Parameters, Serializer};
use crate::{Error, Result};
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

        let (start, sgol) = FullSequence::deserialize(r)?;
        let (end, egol) = FullSequence::deserialize(r)?;

        let end = if end.group_id == 0 {
            if end.object_id > 0 {
                return Err(Error::ErrParseError(
                    ParserErrorCode::ProtocolViolation,
                    "SUBSCRIBE_UPDATE has end_object but no end_group".to_string(),
                ));
            }
            None
        } else {
            let end = if end.object_id == 0 {
                FullSequence {
                    group_id: end.group_id - 1,
                    object_id: u64::MAX,
                }
            } else {
                FullSequence {
                    group_id: end.group_id - 1,
                    object_id: end.object_id - 1,
                }
            };

            if end.group_id < start.group_id {
                return Err(Error::ErrParseError(
                    ParserErrorCode::ProtocolViolation,
                    "End group is less than start group".to_string(),
                ));
            } else if end.group_id == start.group_id && end.object_id < start.object_id {
                return Err(Error::ErrParseError(
                    ParserErrorCode::ProtocolViolation,
                    "End object comes before start object".to_string(),
                ));
            }

            Some(end)
        };

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
                        "AUTHORIZATION_INFO parameter appears twice in SUBSCRIBE_UPDATE"
                            .to_string(),
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
                subscribe_id,

                start_group_object: start,
                end_group_object: end,

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
            let end_group_id = if end_group_object.group_id == u64::MAX {
                if end_group_object.object_id != u64::MAX {
                    return Err(Error::ErrFrameError("Invalid object range".to_string()));
                }
                0
            } else {
                end_group_object.group_id + 1
            };
            let end_object_id = if end_group_object.object_id == u64::MAX {
                0
            } else {
                end_group_object.object_id + 1
            };

            l += FullSequence {
                group_id: end_group_id,
                object_id: end_object_id,
            }
            .serialize(w)?;
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
