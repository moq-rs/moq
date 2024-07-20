use crate::handler::Handler;
use crate::message::message_parser::{MessageParser, MessageParserEvent, ParserErrorCode};
use crate::message::object::ObjectHeader;
use crate::message::{ControlMessage};
use crate::session::session_parameters::{Perspective, SessionParameters};
use crate::{Error, Result, StreamId};
use bytes::{BufMut, Bytes, BytesMut};
use log::trace;
use retty::transport::Transmit;
use std::collections::VecDeque;
use std::time::Instant;
use crate::session::remote_track::RemoteTrackOnObjectFragment;

pub enum StreamEventIn {
    ResetStreamReceived(u64),
    StopSendingReceived(u64),
    WriteSideInDataRecvState,
    MessageParserEvent(MessageParserEvent),
}

pub enum StreamEventOut {
    RemoteTrackOnObjectFragment(RemoteTrackOnObjectFragment),
}

pub struct StreamMessage {
    pub message: BytesMut,
    pub fin: bool,
}

pub struct Stream {
    // none means "incoming stream, and we don't know if it's the control
    // stream or a data stream yet".
    stream_id: StreamId,
    is_control_stream: Option<bool>,
    partial_object: Option<BytesMut>,
    parser: MessageParser,
    session_parameters: SessionParameters,

    eouts: VecDeque<StreamEventOut>,
    routs: VecDeque<Transmit<StreamMessage>>,
    wouts: VecDeque<Transmit<StreamMessage>>,
}

impl Stream {
    pub fn new(
        session_parameters: SessionParameters,
        stream_id: StreamId,
        is_control_stream: Option<bool>,
    ) -> Self {
        Self {
            stream_id,
            is_control_stream,
            partial_object: None,
            parser: MessageParser::new(session_parameters.use_web_transport),
            session_parameters,

            eouts: VecDeque::new(),
            routs: VecDeque::new(),
            wouts: VecDeque::new(),
        }
    }

    pub fn perspective(&self) -> Perspective {
        self.session_parameters.perspective
    }

    fn on_object_message(&mut self, object_header: ObjectHeader, mut payload: Bytes, fin: bool) -> Result<()> {
        if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
            if is_control_stream {
                return Err(Error::ErrParseError(ParserErrorCode::ProtocolViolation,
                                                "Received OBJECT message on control stream".to_string()));
            }
        }
        trace!("{}", format!("{:?} Received OBJECT message on stream {} for subscribe_id {} for
           track alias {} with sequence {}:{} send_order {} forwarding_preference {:?} length {}
           explicit length {} {}",
           self.session_parameters.perspective,
           self.stream_id,
           object_header.subscribe_id,
           object_header.track_alias,
           object_header.group_id,
           object_header.object_id,
           object_header.object_send_order,
           object_header.object_forwarding_preference,
           payload.len(),
           if let Some(&payload_length) = object_header.object_payload_length.as_ref() { payload_length as i64 } else {-1},
           if fin { "F"} else {""},
           ));

        if !self.session_parameters.deliver_partial_objects {
            if !fin {  // Buffer partial object.
                if self.partial_object.is_none() {
                    self.partial_object = Some(BytesMut::new());
                }
                if let Some(partial_object) = self.partial_object.as_mut() {
                    partial_object.put(payload);
                }
                return Ok(());
            }
            if let Some(mut partial_object) = self.partial_object.take() {  // Completes the object
                partial_object.put(payload);
                payload = partial_object.freeze();
            }
        }
        self.eouts.push_back(StreamEventOut::RemoteTrackOnObjectFragment(RemoteTrackOnObjectFragment {
            object_header,
            payload,
            fin,
        }));

        Ok(())
    }
}

impl Handler for Stream {
    type Ein = StreamEventIn;
    type Eout = StreamEventOut;
    type Rin = StreamMessage;
    type Rout = StreamMessage;
    type Win = StreamMessage;
    type Wout = StreamMessage;

    fn handle_read(&mut self, msg: Transmit<Self::Rin>) -> Result<()> {
        self.parser
            .process_data(&mut &msg.message.message[..], msg.message.fin);
        Ok(())
    }

    fn poll_read(&mut self) -> Option<Transmit<Self::Rout>> {
        self.routs.pop_front()
    }

    fn handle_write(&mut self, msg: Transmit<Self::Win>) -> Result<()> {
        self.wouts.push_back(msg);
        Ok(())
    }

    fn poll_write(&mut self) -> Option<Transmit<Self::Wout>> {
        self.wouts.pop_front()
    }

    /// Handles event
    fn handle_event(&mut self, evt: Self::Ein) -> Result<()> {
        match evt {
            StreamEventIn::ResetStreamReceived(error_code) => {
                if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
                    if is_control_stream {
                        return Err(Error::ErrParseError(
                            ParserErrorCode::ProtocolViolation,
                            format!("Control stream reset with error code {}", error_code),
                        ));
                    }
                }
                Ok(())
            }
            StreamEventIn::StopSendingReceived(error_code) => {
                if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
                    if is_control_stream {
                        return Err(Error::ErrParseError(
                            ParserErrorCode::ProtocolViolation,
                            format!("Control stream reset with error code {}", error_code),
                        ));
                    }
                }
                Ok(())
            }
            StreamEventIn::WriteSideInDataRecvState => {
                Ok(())
            }
            StreamEventIn::MessageParserEvent(message_parser_event) => match message_parser_event {
                MessageParserEvent::ParsingError(error_code, reason) => {
                    Err(Error::ErrParseError(
                        error_code,
                        format!("Parse error: {}", reason),
                    ))
                }
                MessageParserEvent::ObjectMessage(object_header, payload, fin) => {
                    self.on_object_message(object_header, payload, fin)
                }
                MessageParserEvent::ControlMessage(control_message) => {
                    /*match control_message {
                        ControlMessage::SubscribeUpdate(subscribe_update) => {}
                        ControlMessage::Subscribe(subscribe) => {}
                        ControlMessage::SubscribeOk(subscribe_ok) => {}
                        ControlMessage::SubscribeError(subscribe_error) => {}
                        ControlMessage::Announce(announce) => {}
                        ControlMessage::AnnounceOk(announce_ok) => {}
                        ControlMessage::AnnounceError(announce_error) => {}
                        ControlMessage::UnAnnounce(unannounce) => {}
                        ControlMessage::UnSubscribe(unsubscribe) => {}
                        ControlMessage::SubscribeDone(subscribe_done) => {}
                        ControlMessage::AnnounceCancel(announce_cancel) => {}
                        ControlMessage::TrackStatusRequest(track_status_request) => {}
                        ControlMessage::TrackStatus(track_status) => {}
                        ControlMessage::GoAway(go_away) => {}
                        ControlMessage::ClientSetup(client_setup) => {}
                        ControlMessage::ServerSetup(server_setup) => {}
                    }*/
                    Ok(())
                }
            }
        }
    }

    /// Polls event
    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.eouts.pop_front()
    }

    /// Handles timeout
    fn handle_timeout(&mut self, _now: Instant) -> Result<()> {
        Ok(())
    }

    /// Polls timeout
    fn poll_timeout(&mut self) -> Option<Instant> {
        None
    }
}
