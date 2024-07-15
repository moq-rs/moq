use crate::message::object::{ObjectForwardingPreference, ObjectHeader, ObjectStatus};
use crate::message::{ControlMessage, MessageType};
use crate::{Error, Result, Serializer};
use bytes::{BufMut, Bytes};

pub struct MessageFramer;

impl MessageFramer {
    pub fn serialize_control_message<W: BufMut>(
        control_message: &ControlMessage,
        w: &mut W,
    ) -> Result<usize> {
        control_message.serialize(w)
    }

    pub fn serialize_object_header<W: BufMut>(
        object_header: &ObjectHeader,
        is_first_in_stream: bool,
        w: &mut W,
    ) -> Result<usize> {
        if object_header.object_payload_length.is_none()
            && !(object_header.object_forwarding_preference == ObjectForwardingPreference::Object
                || object_header.object_forwarding_preference
                    == ObjectForwardingPreference::Datagram)
        {
            return Err(Error::ErrInvalidObjectType("Track or Group forwarding preference requires knowing the object length in advance".to_string()));
        }
        if object_header.object_status != ObjectStatus::Normal {
            if let Some(&object_payload_length) = object_header.object_payload_length.as_ref() {
                if object_payload_length > 0 {
                    return Err(Error::ErrInvalidObjectType(
                        "Object status must be kNormal if payload is non-empty".to_string(),
                    ));
                }
            }
        }

        let mut tl = 0;
        if !is_first_in_stream {
            match object_header.object_forwarding_preference {
                ObjectForwardingPreference::Track => {
                    let object_payload_length = if let Some(&object_payload_length) =
                        object_header.object_payload_length.as_ref()
                    {
                        object_payload_length
                    } else {
                        0
                    };
                    tl += object_header.group_id.serialize(w)?;
                    tl += object_header.object_id.serialize(w)?;
                    tl += object_payload_length.serialize(w)?;
                    if object_payload_length == 0 {
                        tl += (object_header.object_status as u64).serialize(w)?
                    }
                    return Ok(tl);
                }
                ObjectForwardingPreference::Group => {
                    let object_payload_length = if let Some(&object_payload_length) =
                        object_header.object_payload_length.as_ref()
                    {
                        object_payload_length
                    } else {
                        0
                    };
                    tl += object_header.object_id.serialize(w)?;
                    tl += object_payload_length.serialize(w)?;
                    if object_payload_length == 0 {
                        tl += (object_header.object_status as u64).serialize(w)?
                    }
                    return Ok(tl);
                }
                _ => {
                    return Err(Error::ErrInvalidObjectType(
                        "Object or Datagram forwarding_preference must be first in stream"
                            .to_string(),
                    ));
                }
            }
        }

        let message_type = object_header
            .object_forwarding_preference
            .get_message_type();
        match object_header.object_forwarding_preference {
            ObjectForwardingPreference::Track => {
                let object_payload_length = if let Some(&object_payload_length) =
                    object_header.object_payload_length.as_ref()
                {
                    object_payload_length
                } else {
                    0
                };
                tl += message_type.serialize(w)?;
                tl += object_header.subscribe_id.serialize(w)?;
                tl += object_header.track_alias.serialize(w)?;
                tl += object_header.object_send_order.serialize(w)?;
                tl += object_header.group_id.serialize(w)?;
                tl += object_header.object_id.serialize(w)?;
                tl += object_payload_length.serialize(w)?;
                if object_payload_length == 0 {
                    tl += (object_header.object_status as u64).serialize(w)?;
                }
                Ok(tl)
            }
            ObjectForwardingPreference::Group => {
                let object_payload_length = if let Some(&object_payload_length) =
                    object_header.object_payload_length.as_ref()
                {
                    object_payload_length
                } else {
                    0
                };
                tl += message_type.serialize(w)?;
                tl += object_header.subscribe_id.serialize(w)?;
                tl += object_header.track_alias.serialize(w)?;
                tl += object_header.group_id.serialize(w)?;
                tl += object_header.object_send_order.serialize(w)?;
                tl += object_header.object_id.serialize(w)?;
                tl += object_payload_length.serialize(w)?;
                if object_payload_length == 0 {
                    tl += (object_header.object_status as u64).serialize(w)?;
                }
                Ok(tl)
            }
            ObjectForwardingPreference::Object | ObjectForwardingPreference::Datagram => {
                tl += message_type.serialize(w)?;
                tl += object_header.subscribe_id.serialize(w)?;
                tl += object_header.track_alias.serialize(w)?;
                tl += object_header.group_id.serialize(w)?;
                tl += object_header.object_id.serialize(w)?;
                tl += object_header.object_send_order.serialize(w)?;
                tl += (object_header.object_status as u64).serialize(w)?;

                Ok(tl)
            }
        }
    }

    pub fn serialize_object_datagram<W: BufMut>(
        object_header: &ObjectHeader,
        payload: Bytes,
        w: &mut W,
    ) -> Result<usize> {
        if object_header.object_status != ObjectStatus::Normal && !payload.is_empty() {
            return Err(Error::ErrInvalidObjectType(
                "Object status must be kNormal if payload is non-empty".to_string(),
            ));
        }

        let mut tl = 0;
        tl += MessageType::ObjectDatagram.serialize(w)?;
        tl += object_header.subscribe_id.serialize(w)?;
        tl += object_header.track_alias.serialize(w)?;
        tl += object_header.group_id.serialize(w)?;
        tl += object_header.object_id.serialize(w)?;
        tl += object_header.object_send_order.serialize(w)?;
        tl += (object_header.object_status as u64).serialize(w)?;
        tl += payload.serialize(w)?;

        Ok(tl)
    }
}
