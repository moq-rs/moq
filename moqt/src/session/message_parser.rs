use crate::message::announce::Announce;
use crate::message::announce_cancel::AnnounceCancel;
use crate::message::announce_error::AnnounceError;
use crate::message::announce_ok::AnnounceOk;
use crate::message::client_setup::ClientSetup;
use crate::message::go_away::GoAway;
use crate::message::object::{ObjectForwardingPreference, ObjectHeader, ObjectStatus};
use crate::message::server_setup::ServerSetup;
use crate::message::subscribe::Subscribe;
use crate::message::subscribe_done::SubscribeDone;
use crate::message::subscribe_error::SubscribeError;
use crate::message::subscribe_ok::SubscribeOk;
use crate::message::subscribe_update::SubscribeUpdate;
use crate::message::track_status::TrackStatus;
use crate::message::track_status_request::TrackStatusRequest;
use crate::message::unannounce::UnAnnounce;
use crate::message::unsubscribe::UnSubscribe;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::VecDeque;

pub enum ParserErrorCode {
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
    ClientSetupMessage(ClientSetup),
    ServerSetupMessage(ServerSetup),
    SubscribeMessage(Subscribe),
    SubscribeOkMessage(SubscribeOk),
    SubscribeErrorMessage(SubscribeError),
    UnSubscribeMessage(UnSubscribe),
    SubscribeDoneMessage(SubscribeDone),
    SubscribeUpdateMessage(SubscribeUpdate),
    AnnounceMessage(Announce),
    AnnounceOkMessage(AnnounceOk),
    AnnounceErrorMessage(AnnounceError),
    AnnounceCancelMessage(AnnounceCancel),
    TrackStatusRequestMessage(TrackStatusRequest),
    UnAnnounceMessage(UnAnnounce),
    TrackStatusMessage(TrackStatus),
    GoAwayMessage(GoAway),
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
    /// TODO: Figure out what has to happen if the message arrives via
    ///       datagram rather than a stream.
    pub fn process_data<R: Buf>(&mut self, r: &mut R, fin: bool) {
        if self.no_more_data {
            self.parse_error(
                ParserErrorCode::ProtocolViolation,
                "Data after end of stream".to_string(),
            );
        }

        // Check for early fin
        if fin {
            self.no_more_data = true;
            if self.object_payload_in_progress() && self.payload_length_remaining > r.remaining() {
                self.parse_error(
                    ParserErrorCode::ProtocolViolation,
                    "End of stream before complete OBJECT PAYLOAD".to_string(),
                );
                return;
            }
            if !self.buffered_message.is_empty() && !r.has_remaining() {
                self.parse_error(
                    ParserErrorCode::ProtocolViolation,
                    "End of stream before complete message".to_string(),
                );
                return;
            }
        }

        //let reader: &mut dyn Buf;
        let original_buffer_size = self.buffered_message.len();
        // There are three cases: the parser has already delivered an OBJECT header
        // and is now delivering payload; part of a message is in the buffer; or
        // no message is in progress.
        if self.object_payload_in_progress() {
            if let Some(object_metadata) = self.object_metadata.as_ref() {
                // This is additional payload for an OBJECT.
                assert!(self.buffered_message.is_empty());
                if object_metadata.object_payload_length.is_none() {
                    // Deliver the data and exit.
                    self.parser_events
                        .push_back(MessageParserEvent::ObjectMessage(
                            *object_metadata,
                            r.copy_to_bytes(r.remaining()),
                            fin,
                        ));
                    if fin {
                        self.object_metadata = None;
                    }
                    return;
                }
                if r.remaining() < self.payload_length_remaining {
                    // Does not finish the payload; deliver and exit.
                    self.payload_length_remaining -= r.remaining();
                    self.parser_events
                        .push_back(MessageParserEvent::ObjectMessage(
                            *object_metadata,
                            r.copy_to_bytes(r.remaining()),
                            false,
                        ));
                    return;
                }
                // Finishes the payload. Deliver and continue.
                //reader = r;
                self.parser_events
                    .push_back(MessageParserEvent::ObjectMessage(
                        *object_metadata,
                        r.copy_to_bytes(self.payload_length_remaining),
                        true,
                    ));
                self.payload_length_remaining = 0; // Expect a new object.
            } else {
                //reader = r;
            }
        } else if !self.buffered_message.is_empty() {
            self.buffered_message.put(r);
            //reader = &mut self.buffered_message
        } else {
            // No message in progress.
            //reader = r;
        }

        let total_processed = 0;
        /*
        while reader.has_remaining() {
            let message_len = self.process_message(reader, fin);
            if (message_len == 0) {
                if reader.remaining() > MAX_MESSSAGE_HEADER_SIZE {
                    self.parse_error(ParserErrorCode::InternalError, "Cannot parse non-OBJECT messages > 2KB".to_string());
                    return;
                }
                if fin {
                    self.parse_error(ParserErrorCode::ProtocolViolation, "FIN after incomplete message".to_string());
                    return;
                }
                if self.buffered_message.is_empty() {
                    // If the buffer is not empty, |data| has already been copied there.
                    self.buffered_message.put(reader.copy_to_bytes(reader.remaining()));
                }
                break;
            }
            // A message was successfully processed.
            total_processed += message_len;
            //reader->Seek(message_len);
        }*/
        if original_buffer_size > 0 {
            let _ = self.buffered_message.split_to(total_processed);
        }
    }

    /// Provide a separate path for datagrams. Returns the payload bytes, or empty
    /// string_view on error. The caller provides the whole datagram in |data|.
    /// The function puts the object metadata in |object_metadata|.
    pub fn process_datagram<R: Buf>(_r: &mut R, _object_metadata: &ObjectHeader) -> Bytes {
        Bytes::new()
    }

    pub fn poll_event(&mut self) -> Option<MessageParserEvent> {
        self.parser_events.pop_front()
    }

    fn process_message(&mut self, _r: &mut dyn Buf, _fin: bool) -> usize {
        if self.object_stream_initialized() && !self.object_payload_in_progress() {
            // This is a follow-on object in a stream.
            return 0; /*& ProcessObject(reader,
                      GetMessageTypeForForwardingPreference(
                          object_metadata_->forwarding_preference),
                      fin);*/
        }
        0
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
