use crate::message::object::{ObjectForwardingPreference, ObjectHeader, ObjectStatus};
use crate::message::{ControlMessage, MessageType, MAX_MESSSAGE_HEADER_SIZE};
use crate::serde::Deserializer;
use crate::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::VecDeque;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParserErrorCode {
    #[default]
    NoError = 0x0,
    InternalError = 0x1,
    Unauthorized = 0x2,
    ProtocolViolation = 0x3,
    DuplicateTrackAlias = 0x4,
    ParameterLengthMismatch = 0x5,
    GoawayTimeout = 0x10,
}

pub enum MessageParserEvent {
    ParsingError(ParserErrorCode, String),
    ObjectMessage(ObjectHeader, Bytes, bool),
    ControlMessage(ControlMessage),
}

pub struct MessageParser {
    use_web_transport: bool,
    no_more_data: bool, // Fatal error or fin. No more parsing.
    parsing_error: bool,

    buffered_message: BytesMut,

    // Metadata for an object which is delivered in parts.
    // If object_metadata_ is none, nothing has been processed on the stream.
    // If object_metadata_ exists but payload_length is nullopt or
    // payload_length_remaining_ is nonzero, the object payload is in mid-
    // delivery.
    // If object_metadata_ exists and payload_length_remaining_ is zero, an object
    // has been completely delivered and the next object header on the stream has
    // not been delivered.
    // Use object_stream_initialized() and object_payload_in_progress() to keep the
    // state straight.
    object_metadata: Option<ObjectHeader>,
    payload_length_remaining: usize,

    parser_events: VecDeque<MessageParserEvent>,
}

impl MessageParser {
    pub fn new(use_web_transport: bool) -> Self {
        Self {
            use_web_transport,
            no_more_data: false,
            parsing_error: false,

            buffered_message: Default::default(),
            object_metadata: None,
            payload_length_remaining: 0,

            parser_events: VecDeque::new(),
        }
    }

    /// Take a buffer from the transport in |data|. Parse each complete message and
    /// call the appropriate visitor function. If |fin| is true, there
    /// is no more data arriving on the stream, so the parser will deliver any
    /// message encoded as to run to the end of the stream.
    /// All bytes can be freed. Calls OnParsingError() when there is a parsing
    /// error.
    /// Any calls after sending |fin| = true will be ignored.
    pub fn process_data<R: Buf>(&mut self, buf: &mut R, fin: bool) {
        if self.no_more_data {
            self.parse_error(
                ParserErrorCode::ProtocolViolation,
                "Data after end of stream".to_string(),
            );
        }

        // Check for early fin
        if fin {
            self.no_more_data = true;
            if self.object_payload_in_progress() && self.payload_length_remaining > buf.remaining()
            {
                self.parse_error(
                    ParserErrorCode::ProtocolViolation,
                    "End of stream before complete OBJECT PAYLOAD".to_string(),
                );
                return;
            }
            if !self.buffered_message.is_empty() && !buf.has_remaining() {
                self.parse_error(
                    ParserErrorCode::ProtocolViolation,
                    "End of stream before complete message".to_string(),
                );
                return;
            }
        }

        self.buffered_message.put(buf);

        // There are three cases: the parser has already delivered an OBJECT header
        // and is now delivering payload; part of a message is in the buffer; or
        // no message is in progress.
        if self.object_payload_in_progress() {
            if let Some(object_metadata) = self.object_metadata.as_ref() {
                // This is additional payload for an OBJECT.
                if object_metadata.object_payload_length.is_none() {
                    // Deliver the data and exit.
                    self.parser_events
                        .push_back(MessageParserEvent::ObjectMessage(
                            *object_metadata,
                            self.buffered_message
                                .copy_to_bytes(self.buffered_message.remaining()),
                            fin,
                        ));
                    if fin {
                        self.object_metadata = None;
                    }
                    return;
                }
                if self.buffered_message.remaining() < self.payload_length_remaining {
                    // Does not finish the payload; deliver and exit.
                    self.payload_length_remaining -= self.buffered_message.remaining();
                    self.parser_events
                        .push_back(MessageParserEvent::ObjectMessage(
                            *object_metadata,
                            self.buffered_message
                                .copy_to_bytes(self.buffered_message.remaining()),
                            false,
                        ));
                    return;
                }
                // Finishes the payload. Deliver and continue.
                self.parser_events
                    .push_back(MessageParserEvent::ObjectMessage(
                        *object_metadata,
                        self.buffered_message
                            .copy_to_bytes(self.payload_length_remaining),
                        true,
                    ));
                self.payload_length_remaining = 0; // Expect a new object.
            }
        }

        while self.buffered_message.has_remaining() {
            let message_len = self.process_message(fin);
            if message_len == 0 {
                if self.buffered_message.remaining() > MAX_MESSSAGE_HEADER_SIZE {
                    self.parse_error(
                        ParserErrorCode::InternalError,
                        "Cannot parse non-OBJECT messages > 2KB".to_string(),
                    );
                    return;
                }
                if fin {
                    self.parse_error(
                        ParserErrorCode::ProtocolViolation,
                        "FIN after incomplete message".to_string(),
                    );
                    return;
                }
                break;
            }
            self.buffered_message.advance(message_len);
        }
    }

    /// Provide a separate path for datagrams. Returns the ObjectHeader and payload bytes
    pub fn process_datagram<R: Buf>(r: &mut R) -> Result<(ObjectHeader, Bytes)> {
        let (object_header, _) = MessageParser::parse_object_header(r)?;
        if object_header.object_forwarding_preference != ObjectForwardingPreference::Datagram {
            return Err(Error::ErrProtocolViolation("invalid datagram".to_string()));
        }
        Ok((object_header, r.copy_to_bytes(r.remaining())))
    }

    pub fn poll_event(&mut self) -> Option<MessageParserEvent> {
        self.parser_events.pop_front()
    }

    fn process_message(&mut self, fin: bool) -> usize {
        if self.object_stream_initialized() && !self.object_payload_in_progress() {
            // This is a follow-on object in a stream.
            if let Some(object_metadata) = self.object_metadata.as_ref() {
                return self.process_object(
                    object_metadata
                        .object_forwarding_preference
                        .get_message_type(),
                    fin,
                );
            }
        }
        let mut mt_reader = self.buffered_message.as_ref();
        let message_type = match MessageType::deserialize(&mut mt_reader) {
            Ok((message_type, _)) => message_type,
            Err(_) => return 0,
        };

        if message_type == MessageType::ObjectDatagram {
            self.parse_error(
                ParserErrorCode::ProtocolViolation,
                "Received OBJECT_DATAGRAM on strea".to_string(),
            );
            0
        } else if message_type == MessageType::ObjectStream
            || message_type == MessageType::StreamHeaderTrack
            || message_type == MessageType::StreamHeaderGroup
        {
            self.process_object(message_type, fin)
        } else {
            let mut msg_reader = self.buffered_message.as_ref();
            let (control_message, message_len) = match ControlMessage::deserialize(&mut msg_reader)
            {
                Ok((control_message, message_len)) => (control_message, message_len),
                Err(_) => return 0,
            };
            self.parser_events
                .push_back(MessageParserEvent::ControlMessage(control_message));
            message_len
        }
    }

    fn process_object(&mut self, message_type: MessageType, fin: bool) -> usize {
        let mut processed_data = 0;
        assert!(!self.object_payload_in_progress());
        if !self.object_stream_initialized() {
            let mut oh_reader = self.buffered_message.as_ref();
            let (object_metadata, obl) = match MessageParser::parse_object_header(&mut oh_reader) {
                Ok((object_metadata, obl)) => (object_metadata, obl),
                Err(err) => {
                    if let Error::ErrProtocolViolation(reason) = &err {
                        self.parse_error(ParserErrorCode::ProtocolViolation, reason.to_string());
                    }
                    return 0;
                }
            };
            self.object_metadata = Some(object_metadata);
            processed_data += obl;
        }

        let mut payload_reader = &self.buffered_message.as_ref()[processed_data..];
        match MessageParser::process_object_payload(
            &mut self.parser_events,
            &mut self.object_metadata,
            &mut self.payload_length_remaining,
            &mut payload_reader,
            message_type,
            fin,
        ) {
            Ok(prl) => {
                processed_data += prl;
            }
            Err(err) => {
                if let Error::ErrProtocolViolation(reason) = &err {
                    self.parse_error(ParserErrorCode::ProtocolViolation, reason.to_string());
                }
            }
        };

        processed_data
    }

    fn parse_object_header<R: Buf>(r: &mut R) -> Result<(ObjectHeader, usize)> {
        let (message_type, mtl) = MessageType::deserialize(r)?;
        let (subscribe_id, sil) = u64::deserialize(r)?;
        let (track_alias, tal) = u64::deserialize(r)?;
        let (group_id, gil) = if message_type != MessageType::StreamHeaderTrack {
            u64::deserialize(r)?
        } else {
            (0, 0)
        };
        let (object_id, oil) = if message_type != MessageType::StreamHeaderTrack
            && message_type != MessageType::StreamHeaderGroup
        {
            u64::deserialize(r)?
        } else {
            (0, 0)
        };
        let (object_send_order, osol) = u64::deserialize(r)?;
        let (status, osl) = if message_type == MessageType::ObjectStream
            || message_type == MessageType::ObjectDatagram
        {
            u64::deserialize(r)?
        } else {
            (0, 0)
        };
        let object_status: ObjectStatus = status.into();
        let object_forwarding_preference: ObjectForwardingPreference =
            message_type.get_object_forwarding_preference()?;

        Ok((
            ObjectHeader {
                subscribe_id,
                track_alias,
                group_id,
                object_id,
                object_send_order,
                object_status,
                object_forwarding_preference,
                object_payload_length: None,
            },
            mtl + sil + tal + gil + oil + osol + osl,
        ))
    }

    fn process_object_payload<R: Buf>(
        parser_events: &mut VecDeque<MessageParserEvent>,
        object_header: &mut Option<ObjectHeader>,
        payload_length_remaining: &mut usize,
        r: &mut R,
        message_type: MessageType,
        fin: bool,
    ) -> Result<usize> {
        // At this point, enough data has been processed to store in object_metadata_,
        // even if there's nothing else in the buffer.
        assert!(*payload_length_remaining == 0);
        let mut total_len = 0;
        if message_type == MessageType::StreamHeaderTrack {
            let (group_id, gil) = u64::deserialize(r)?;
            total_len += gil;
            if let Some(object_metadata) = object_header.as_mut() {
                object_metadata.group_id = group_id;
            }
        }
        if message_type == MessageType::StreamHeaderTrack
            || message_type == MessageType::StreamHeaderGroup
        {
            let (object_id, oil) = u64::deserialize(r)?;
            total_len += oil;

            let (object_payload_length, opl) = u64::deserialize(r)?;
            total_len += opl;

            let mut status = 0; // Defaults to kNormal.
            if object_payload_length == 0 {
                let sl;
                (status, sl) = u64::deserialize(r)?;
                total_len += sl;
            }

            if let Some(object_metadata) = object_header.as_mut() {
                object_metadata.object_id = object_id;
                object_metadata.object_payload_length = Some(object_payload_length);
                object_metadata.object_status = status.into();
            }
        }

        if let Some(object_metadata) = object_header.as_ref() {
            if object_metadata.object_status == ObjectStatus::Invalid {
                return Err(Error::ErrProtocolViolation(
                    "Invalid object status".to_string(),
                ));
            }
            if object_metadata.object_status != ObjectStatus::Normal {
                // It is impossible to express an explicit length with this status.
                if (message_type == MessageType::ObjectStream
                    || message_type == MessageType::ObjectDatagram)
                    && r.has_remaining()
                {
                    // There is additional data in the stream/datagram, which is an error.
                    return Err(Error::ErrProtocolViolation(
                        "Object with non-normal status has payload".to_string(),
                    ));
                }
                parser_events.push_back(MessageParserEvent::ObjectMessage(
                    *object_metadata,
                    Bytes::new(),
                    true,
                ));
                return Ok(total_len);
            }

            let has_length = object_metadata.object_payload_length.is_some();
            let payload_length = if let Some(object_payload_length) =
                object_metadata.object_payload_length.as_ref()
            {
                *object_payload_length as usize
            } else {
                0
            };
            let mut payload_to_draw = r.remaining();
            if fin && has_length && payload_length > r.remaining() {
                return Err(Error::ErrProtocolViolation(
                    "Received FIN mid-payload".to_string(),
                ));
            }
            let received_complete_message = fin || (has_length && payload_length <= r.remaining());
            if received_complete_message && has_length && payload_length < r.remaining() {
                payload_to_draw = payload_length;
            }
            // The error case where there's a fin before the explicit length is complete
            // is handled in ProcessData() in two separate places. Even though the
            // message is "done" if fin regardless of has_length, it's bad to report to
            // the application that the object is done if it hasn't reached the promised
            // length.
            parser_events.push_back(MessageParserEvent::ObjectMessage(
                *object_metadata,
                r.copy_to_bytes(payload_to_draw),
                received_complete_message,
            ));
            *payload_length_remaining = if has_length {
                payload_length - payload_to_draw
            } else {
                0
            };

            total_len += payload_to_draw;
        }

        Ok(total_len)
    }

    fn parse_error(&mut self, error_code: ParserErrorCode, error_reason: String) {
        if self.parsing_error {
            return; // Don't send multiple parse errors.
        }
        self.no_more_data = true;
        self.parsing_error = true;
        self.parser_events
            .push_back(MessageParserEvent::ParsingError(error_code, error_reason));
    }

    // Simplify understanding of state.
    // Returns true if the stream has delivered all object metadata common to all
    // objects on that stream.
    fn object_stream_initialized(&self) -> bool {
        self.object_metadata.is_some()
    }

    // Returns true if the stream has delivered all metadata but not all payload
    // for the most recent object.
    fn object_payload_in_progress(&self) -> bool {
        if let Some(object_metadata) = self.object_metadata.as_ref() {
            object_metadata.object_status == ObjectStatus::Normal
                && (object_metadata.object_forwarding_preference
                    == ObjectForwardingPreference::Object
                    || object_metadata.object_forwarding_preference
                        == ObjectForwardingPreference::Datagram
                    || self.payload_length_remaining > 0)
        } else {
            false
        }
    }
}
