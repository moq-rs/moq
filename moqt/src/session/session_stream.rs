use crate::handler::Handler;
use crate::message::client_setup::ClientSetup;
use crate::message::message_framer::MessageFramer;
use crate::message::message_parser::{ErrorCode, MessageParser, MessageParserEvent};
use crate::message::object::ObjectHeader;
use crate::message::server_setup::ServerSetup;
use crate::message::{ControlMessage, Role};
use crate::session::remote_track::RemoteTrackOnObjectFragment;
use crate::session::session_parameters::{Perspective, SessionParameters};
use crate::{Error, Result, StreamId};
use bytes::{BufMut, Bytes, BytesMut};
use log::{info, trace};
use retty::transport::{Transmit, TransportContext};
use std::collections::VecDeque;
use std::time::Instant;

pub enum StreamEventIn {
    ResetStreamReceived(u64),
    StopSendingReceived(u64),
    WriteSideInDataRecvState,
    MessageParserEvent(MessageParserEvent),
}

pub enum StreamEventOut {
    RemoteTrackOnObjectFragment(RemoteTrackOnObjectFragment),

    SessionEstablished(Option<Role>, Option<String>),
    SessionTerminated,
    SessionDeleted,
    IncomingAnnounce,
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
    transport: TransportContext,
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
        transport: TransportContext,
    ) -> Self {
        Self {
            stream_id,
            is_control_stream,
            transport,
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

    fn on_object_message(
        &mut self,
        object_header: ObjectHeader,
        mut payload: Bytes,
        fin: bool,
    ) -> Result<()> {
        if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
            if is_control_stream {
                return Err(Error::ErrStreamError(
                    ErrorCode::ProtocolViolation,
                    "Received OBJECT message on control stream".to_string(),
                ));
            }
        }
        trace!(
            "{}",
            format!(
                "{:?} Received OBJECT message on stream {} for subscribe_id {} for
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
                if let Some(&payload_length) = object_header.object_payload_length.as_ref() {
                    payload_length as i64
                } else {
                    -1
                },
                if fin { "F" } else { "" },
            )
        );

        if !self.session_parameters.deliver_partial_objects {
            if !fin {
                // Buffer partial object.
                if self.partial_object.is_none() {
                    self.partial_object = Some(BytesMut::new());
                }
                if let Some(partial_object) = self.partial_object.as_mut() {
                    partial_object.put(payload);
                }
                return Ok(());
            }
            if let Some(mut partial_object) = self.partial_object.take() {
                // Completes the object
                partial_object.put(payload);
                payload = partial_object.freeze();
            }
        }
        self.eouts
            .push_back(StreamEventOut::RemoteTrackOnObjectFragment(
                RemoteTrackOnObjectFragment {
                    object_header,
                    payload,
                    fin,
                },
            ));

        Ok(())
    }

    fn on_client_setup_message(&mut self, client_setup: ClientSetup) -> Result<()> {
        if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
            if !is_control_stream {
                return Err(Error::ErrStreamError(
                    ErrorCode::ProtocolViolation,
                    "Received SETUP on non-control stream".to_string(),
                ));
            }
        } else {
            self.is_control_stream = Some(true);
        }
        if self.perspective() == Perspective::Client {
            return Err(Error::ErrStreamError(
                ErrorCode::ProtocolViolation,
                "Received CLIENT_SETUP from server".to_string(),
            ));
        }
        if !client_setup
            .supported_versions
            .contains(&self.session_parameters.version)
        {
            return Err(Error::ErrStreamError(
                ErrorCode::ProtocolViolation,
                format!(
                    "Version mismatch: expected {:?}",
                    self.session_parameters.version
                ),
            ));
        }
        info!("{:?} Received the SETUP message", self.perspective());
        if self.session_parameters.perspective == Perspective::Server {
            let response = ServerSetup {
                supported_version: self.session_parameters.version,
                role: Some(Role::PubSub),
            };
            let mut message = BytesMut::new();
            MessageFramer::serialize_control_message(
                ControlMessage::ServerSetup(response),
                &mut message,
            )?;
            self.handle_write(Transmit {
                now: Instant::now(),
                transport: self.transport,
                message: StreamMessage { message, fin: true },
            })?;
            info!("{:?} Sent the SETUP message", self.perspective());
        }
        self.eouts.push_back(StreamEventOut::SessionEstablished(
            client_setup.role,
            client_setup.path,
        ));
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
                        return Err(Error::ErrStreamError(
                            ErrorCode::ProtocolViolation,
                            format!("Control stream reset with error code {}", error_code),
                        ));
                    }
                }
                Ok(())
            }
            StreamEventIn::StopSendingReceived(error_code) => {
                if let Some(&is_control_stream) = self.is_control_stream.as_ref() {
                    if is_control_stream {
                        return Err(Error::ErrStreamError(
                            ErrorCode::ProtocolViolation,
                            format!("Control stream reset with error code {}", error_code),
                        ));
                    }
                }
                Ok(())
            }
            StreamEventIn::WriteSideInDataRecvState => Ok(()),
            StreamEventIn::MessageParserEvent(message_parser_event) => match message_parser_event {
                MessageParserEvent::ParsingError(error_code, reason) => Err(Error::ErrStreamError(
                    error_code,
                    format!("Parse error: {}", reason),
                )),
                MessageParserEvent::ObjectMessage(object_header, payload, fin) => {
                    self.on_object_message(object_header, payload, fin)
                }
                MessageParserEvent::ControlMessage(_control_message) => {
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
            },
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
