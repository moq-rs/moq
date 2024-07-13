use crate::message::object::{ObjectHeader, ObjectStatus};
use crate::message::{ControlMessage, MessageType, MAX_MESSSAGE_HEADER_SIZE};
use crate::{Deserializer, Error, Result, Serializer, VarInt};
use bytes::{Buf, BufMut};

enum MessageStructuredData {
    Control(ControlMessage),
    Object(ObjectHeader),
}

trait TestMessageBase {
    // Returns a copy of the structured data for the message.
    fn structured_data(&self) -> MessageStructuredData;

    // Compares |values| to the derived class's structured data to make sure
    // they are equal.
    fn equal_field_values(&self, values: &MessageStructuredData) -> bool;

    // Expand all varints in the message. This is pure virtual because each
    // message has a different layout of varints.
    fn expand_varints(&mut self) -> Result<()>;
}

struct TestMessage {
    message_type: MessageType,
    wire_image: [u8; MAX_MESSSAGE_HEADER_SIZE + 20],
    wire_image_size: usize,
}

impl TestMessage {
    fn new(message_type: MessageType) -> Self {
        Self {
            message_type,
            wire_image: [0u8; MAX_MESSSAGE_HEADER_SIZE + 20],
            wire_image_size: 0,
        }
    }

    fn message_type(&self) -> MessageType {
        self.message_type
    }

    // The total actual size of the message.
    fn total_message_size(&self) -> usize {
        self.wire_image_size
    }

    fn packet_sample(&self) -> &[u8] {
        &self.wire_image[..self.wire_image_size]
    }

    fn set_wire_image_size(&mut self, wire_image_size: usize) {
        self.wire_image_size = wire_image_size;
    }

    fn set_wire_image(&mut self, wire_image: &[u8], wire_image_size: usize) {
        self.wire_image[..wire_image_size].copy_from_slice(&wire_image[..wire_image_size]);
        self.wire_image_size = wire_image_size;
    }

    fn write_var_int62with_forced_length<W: BufMut>(
        v: u64,
        w: &mut W,
        write_length: usize,
    ) -> Result<usize> {
        let vi: VarInt = v.try_into()?;
        let min_length = vi.size();

        if write_length == min_length {
            vi.serialize(w)
        } else if write_length == 2 {
            w.put_u8(0b01000000);
            w.put_u8(v as u8);
            Ok(2)
        } else if write_length == 4 {
            w.put_u8(0b10000000);
            w.put_u8(0);
            w.put_u16(v as u16);
            Ok(4)
        } else if write_length == 8 {
            w.put_u8(0b11000000);
            w.put_u8(0);
            w.put_u16(0);
            w.put_u32(v as u32);
            Ok(8)
        } else {
            Err(Error::ErrBufferTooShort)
        }
    }

    // Expands all the varints in the message, alternating between making them 2,
    // 4, and 8 bytes long. Updates length fields accordingly.
    // Each character in |varints| corresponds to a byte in the original message.
    // If there is a 'v', it is a varint that should be expanded. If '-', skip
    // to the next byte.
    fn expand_varints_impl(&mut self, varints: &[u8]) -> Result<()> {
        let mut next_varint_len = 2;
        let mut reader = &self.wire_image[..self.wire_image_size];
        let mut writer = vec![];
        let mut i = 0;
        while reader.has_remaining() {
            if i >= varints.len() || varints[i] == b'-' {
                i += 1;

                writer.put_u8(reader.get_u8());
                continue;
            }
            let (value, _) = u64::deserialize(&mut reader)?;
            let _ = TestMessage::write_var_int62with_forced_length(
                value,
                &mut writer,
                next_varint_len,
            )?;
            next_varint_len *= 2;
            if next_varint_len == 16 {
                next_varint_len = 2;
            }
        }
        self.wire_image[0..writer.len()].copy_from_slice(&writer[..]);
        self.wire_image_size = writer.len();
        Ok(())
    }
}

struct TestObjectMessage {
    base: TestMessage,
    object_header: ObjectHeader,
}

impl TestObjectMessage {
    fn new(message_type: MessageType) -> Self {
        Self {
            base: TestMessage::new(message_type),
            object_header: ObjectHeader {
                subscribe_id: 3,
                track_alias: 4,
                group_id: 5,
                object_id: 6,
                object_send_order: 7,
                object_status: ObjectStatus::Normal,
                object_forwarding_preference: message_type
                    .get_object_forwarding_preference()
                    .unwrap(),
                object_payload_length: None,
            },
        }
    }
}

impl TestMessageBase for TestObjectMessage {
    fn structured_data(&self) -> MessageStructuredData {
        MessageStructuredData::Object(self.object_header)
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        let cast = if let MessageStructuredData::Object(object_header) = values {
            object_header
        } else {
            return false;
        };
        if cast.subscribe_id != self.object_header.subscribe_id {
            return false;
        }
        if cast.track_alias != self.object_header.track_alias {
            return false;
        }
        if cast.group_id != self.object_header.group_id {
            return false;
        }
        if cast.object_id != self.object_header.object_id {
            return false;
        }
        if cast.object_send_order != self.object_header.object_send_order {
            return false;
        }
        if cast.object_status != self.object_header.object_status {
            return false;
        }
        if cast.object_forwarding_preference != self.object_header.object_forwarding_preference {
            return false;
        }
        if cast.object_payload_length != self.object_header.object_payload_length {
            return false;
        }
        true
    }

    fn expand_varints(&mut self) -> Result<()> {
        todo!()
    }
}

struct TestObjectStreamMessage {
    base: TestObjectMessage,
    raw_packet: Vec<u8>,
}

impl TestObjectStreamMessage {
    fn new() -> Self {
        Self {
            base: TestObjectMessage::new(MessageType::ObjectStream),
            raw_packet: vec![
                0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x00, // varints
                0x66, 0x6f, 0x6f, // payload = "foo"
            ],
        }
    }
}

impl TestMessageBase for TestObjectStreamMessage {
    fn structured_data(&self) -> MessageStructuredData {
        self.base.structured_data()
    }

    fn equal_field_values(&self, values: &MessageStructuredData) -> bool {
        self.base.equal_field_values(values)
    }

    fn expand_varints(&mut self) -> Result<()> {
        self.base.base.expand_varints_impl("vvvvvvv---".as_bytes())
    }
}
