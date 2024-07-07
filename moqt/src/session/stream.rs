use crate::handler::Handler;
use crate::message::message_parser::MessageParserEvent;
use crate::session::session_parameters::{Perspective, SessionParameters};
use bytes::BytesMut;
use retty::transport::Transmit;
use std::collections::VecDeque;
use std::time::Instant;

pub struct Stream {
    session_parameter: SessionParameters,
    // none means "incoming stream, and we don't know if it's the control
    // stream or a data stream yet".
    is_control_stream: Option<bool>,
    partial_object: BytesMut,

    routs: VecDeque<StreamMessage>,
    wouts: VecDeque<StreamMessage>,
}

impl Stream {
    pub fn new(session_parameter: SessionParameters, is_control_stream: Option<bool>) -> Self {
        Self {
            session_parameter,
            is_control_stream,
            partial_object: Default::default(),
            routs: VecDeque::new(),
            wouts: VecDeque::new(),
        }
    }

    pub fn perspective(&self) -> Perspective {
        self.session_parameter.perspective
    }

    /// Sends a control message, or buffers it if there is insufficient flow
    /// control credit.
    fn send_or_buffer_message(&mut self, message: BytesMut, fin: bool) {
        self.wouts.push_back(StreamMessage { message, fin });
    }
}

pub enum StreamEvent {
    // General Messages
    ResetStreamReceived(u64),
    StopSendingReceived(u64),
    WriteSideInDataRecvState,

    MessageParserEvent(MessageParserEvent),
}

pub struct StreamMessage {
    pub message: BytesMut,
    pub fin: bool,
}

impl Handler for Stream {
    type Ein = StreamEvent;
    type Eout = StreamEvent;
    type Rin = StreamMessage;
    type Rout = StreamMessage;
    type Win = StreamMessage;
    type Wout = StreamMessage;

    fn handle_read(&mut self, _msg: Transmit<Self::Rin>) -> crate::Result<()> {
        todo!()
    }

    fn poll_read(&mut self) -> Option<Transmit<Self::Rout>> {
        todo!()
    }

    fn handle_write(&mut self, _msg: Transmit<Self::Win>) -> crate::Result<()> {
        todo!()
    }

    fn poll_write(&mut self) -> Option<Transmit<Self::Wout>> {
        todo!()
    }

    /// Handles event
    fn handle_event(&mut self, _evt: Self::Ein) -> crate::Result<()> {
        Ok(())
    }

    /// Polls event
    fn poll_event(&mut self) -> Option<Self::Eout> {
        None
    }

    /// Handles timeout
    fn handle_timeout(&mut self, _now: Instant) -> crate::Result<()> {
        Ok(())
    }

    /// Polls timeout
    fn poll_timeout(&mut self) -> Option<Instant> {
        None
    }
}
