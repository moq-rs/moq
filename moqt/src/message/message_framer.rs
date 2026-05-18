use crate::message::object::{ObjectForwardingPreference, ObjectHeader, ObjectStatus};
use crate::message::{ControlMessage, MessageType};
use crate::{Error, Result, Serializer};
use bytes::{BufMut, Bytes};

const FETCH_STREAM_TYPE: u64 = 0x05;
const FETCH_HAS_OBJECT_ID: u64 = 0x04;
const FETCH_HAS_GROUP_ID: u64 = 0x08;
const FETCH_HAS_PRIORITY: u64 = 0x10;
const FETCH_HAS_EXTENSIONS: u64 = 0x20;
const FETCH_IS_DATAGRAM_LIKE: u64 = 0x40;

pub struct MessageFramer;

impl MessageFramer {
    pub fn serialize_control_message<W: BufMut>(
        control_message: ControlMessage,
        w: &mut W,
    ) -> Result<usize> {
        control_message.serialize(w)
    }

    pub fn serialize_object_header<W: BufMut>(
        object_header: ObjectHeader,
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

    pub fn serialize_object<W: BufMut>(
        object_header: ObjectHeader,
        is_first_in_stream: bool,
        payload: Bytes,
        w: &mut W,
    ) -> Result<usize> {
        let mut adjusted_object_header = object_header;
        adjusted_object_header.object_payload_length = Some(payload.len() as u64);
        let mut tl =
            MessageFramer::serialize_object_header(adjusted_object_header, is_first_in_stream, w)?;
        tl += payload.serialize(w)?;
        Ok(tl)
    }

    pub fn serialize_fetch_object<W: BufMut>(
        object_header: ObjectHeader,
        is_first_in_stream: bool,
        extension_headers: Bytes,
        payload: Bytes,
        w: &mut W,
    ) -> Result<usize> {
        Self::serialize_fetch_object_with_previous(
            object_header,
            if is_first_in_stream {
                None
            } else {
                Some(object_header)
            },
            extension_headers,
            payload,
            w,
        )
    }

    pub fn serialize_fetch_object_with_previous<W: BufMut>(
        object_header: ObjectHeader,
        previous_object: Option<ObjectHeader>,
        extension_headers: Bytes,
        payload: Bytes,
        w: &mut W,
    ) -> Result<usize> {
        if object_header.object_status != ObjectStatus::Normal {
            return Err(Error::ErrInvalidObjectType(
                "fetch stream objects only support normal status".to_string(),
            ));
        }
        if object_header.object_send_order > u64::from(u8::MAX) {
            return Err(Error::ErrInvalidObjectType(
                "fetch stream priority must fit in one byte".to_string(),
            ));
        }

        let mut tl = 0;
        if previous_object.is_none() {
            tl += FETCH_STREAM_TYPE.serialize(w)?;
            tl += object_header.subscribe_id.serialize(w)?;
        }
        let mut flags = FETCH_IS_DATAGRAM_LIKE;
        if let Some(previous_object) = previous_object {
            if object_header.group_id != previous_object.group_id {
                flags |= FETCH_HAS_GROUP_ID;
            }
            if object_header.object_id != previous_object.object_id + 1 {
                flags |= FETCH_HAS_OBJECT_ID;
            }
            if object_header.object_send_order != previous_object.object_send_order {
                flags |= FETCH_HAS_PRIORITY;
            }
        } else {
            flags |= FETCH_HAS_GROUP_ID | FETCH_HAS_OBJECT_ID | FETCH_HAS_PRIORITY;
        }
        if !extension_headers.is_empty() {
            flags |= FETCH_HAS_EXTENSIONS;
        }
        tl += flags.serialize(w)?;
        if (flags & FETCH_HAS_GROUP_ID) != 0 {
            tl += object_header.group_id.serialize(w)?;
        }
        if (flags & FETCH_HAS_OBJECT_ID) != 0 {
            tl += object_header.object_id.serialize(w)?;
        }
        if (flags & FETCH_HAS_PRIORITY) != 0 {
            if w.remaining_mut() < 1 {
                return Err(Error::ErrBufferTooShort);
            }
            w.put_u8(object_header.object_send_order as u8);
            tl += 1;
        }
        if (flags & FETCH_HAS_EXTENSIONS) != 0 {
            tl += extension_headers.len().serialize(w)?;
            tl += extension_headers.serialize(w)?;
        }
        tl += (payload.len() as u64).serialize(w)?;
        tl += payload.serialize(w)?;
        Ok(tl)
    }

    pub fn serialize_object_datagram<W: BufMut>(
        object_header: ObjectHeader,
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
