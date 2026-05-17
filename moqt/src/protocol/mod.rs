use crate::message::client_setup::ClientSetup;
use crate::message::message_framer::MessageFramer;
use crate::message::{ControlMessage, Role, Version};
use crate::{Result, StreamId};
use bytes::{Bytes, BytesMut};
use sansio::Protocol;
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Perspective {
    #[default]
    Server,
    Client,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub(crate) struct Config {
    pub version: Version,
    pub perspective: Perspective,
    pub use_web_transport: bool,
    pub path: String,
    pub deliver_partial_objects: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum StreamPurpose {
    Control,
    Data,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ReadInput {
    StreamData {
        stream_id: StreamId,
        data: Bytes,
        fin: bool,
    },
    Datagram(Bytes),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ReadOutput {
    StreamData {
        stream_id: StreamId,
        data: Bytes,
        fin: bool,
    },
    Datagram(Bytes),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Command {
    Close { code: u64, reason: String },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum EventIn {
    TransportConnected,
    TransportClosed,
    StreamOpened {
        stream_id: StreamId,
        bidi: bool,
        local: bool,
    },
    StreamClosed {
        stream_id: StreamId,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum EventOut {
    SessionEstablished,
    SessionTerminated,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum WriteOutput {
    OpenBiStream {
        purpose: StreamPurpose,
    },
    SendStream {
        stream_id: StreamId,
        bytes: BytesMut,
        fin: bool,
    },
    SendDatagram(Bytes),
    Close {
        code: u64,
        reason: String,
    },
}

/// New SANS-I/O session core skeleton.
///
/// This sits alongside the older `Handler`-based session code while the rewrite
/// is in progress. It currently covers only the earliest lifecycle needed to
/// bootstrap a client control stream and emit `CLIENT_SETUP`.
pub(crate) struct SessionCore {
    config: Config,
    control_stream_id: Option<StreamId>,
    routs: VecDeque<ReadOutput>,
    wouts: VecDeque<WriteOutput>,
    eouts: VecDeque<EventOut>,
}

impl SessionCore {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            control_stream_id: None,
            routs: VecDeque::new(),
            wouts: VecDeque::new(),
            eouts: VecDeque::new(),
        }
    }

    fn send_client_setup(&mut self, stream_id: StreamId) -> Result<()> {
        let mut client_setup = ClientSetup {
            supported_versions: vec![self.config.version],
            role: Some(Role::PubSub),
            path: None,
            uses_web_transport: self.config.use_web_transport,
        };
        if !self.config.use_web_transport {
            client_setup.path = Some(self.config.path.clone());
        }

        let mut bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(client_setup),
            &mut bytes,
        )?;
        self.wouts.push_back(WriteOutput::SendStream {
            stream_id,
            bytes,
            fin: true,
        });
        Ok(())
    }
}

impl Protocol<ReadInput, Command, EventIn> for SessionCore {
    type Rout = ReadOutput;
    type Wout = WriteOutput;
    type Eout = EventOut;
    type Error = crate::Error;
    type Time = Instant;

    fn handle_read(&mut self, msg: ReadInput) -> Result<()> {
        self.routs.push_back(match msg {
            ReadInput::StreamData {
                stream_id,
                data,
                fin,
            } => ReadOutput::StreamData {
                stream_id,
                data,
                fin,
            },
            ReadInput::Datagram(bytes) => ReadOutput::Datagram(bytes),
        });
        Ok(())
    }

    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.routs.pop_front()
    }

    fn handle_write(&mut self, msg: Command) -> Result<()> {
        match msg {
            Command::Close { code, reason } => {
                self.wouts.push_back(WriteOutput::Close { code, reason });
            }
        }
        Ok(())
    }

    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.wouts.pop_front()
    }

    fn handle_event(&mut self, evt: EventIn) -> Result<()> {
        match evt {
            EventIn::TransportConnected => {
                if self.config.perspective == Perspective::Client
                    && self.control_stream_id.is_none()
                {
                    self.wouts.push_back(WriteOutput::OpenBiStream {
                        purpose: StreamPurpose::Control,
                    });
                }
            }
            EventIn::TransportClosed => {
                self.control_stream_id = None;
                self.eouts.push_back(EventOut::SessionTerminated);
            }
            EventIn::StreamOpened {
                stream_id,
                bidi,
                local,
            } => {
                if self.config.perspective == Perspective::Client
                    && local
                    && bidi
                    && self.control_stream_id.is_none()
                {
                    self.control_stream_id = Some(stream_id);
                    self.send_client_setup(stream_id)?;
                }
            }
            EventIn::StreamClosed { stream_id } => {
                if self.control_stream_id == Some(stream_id) {
                    self.control_stream_id = None;
                }
            }
        }
        Ok(())
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.eouts.pop_front()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::message_parser::MessageParser;

    fn client_config(use_web_transport: bool) -> Config {
        Config {
            version: Version::Draft04,
            perspective: Perspective::Client,
            use_web_transport,
            path: "/moq".to_string(),
            deliver_partial_objects: false,
        }
    }

    #[test]
    fn client_transport_connected_opens_control_stream() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(true));

        protocol.handle_event(EventIn::TransportConnected)?;

        assert_eq!(
            protocol.poll_write(),
            Some(WriteOutput::OpenBiStream {
                purpose: StreamPurpose::Control
            })
        );
        assert_eq!(protocol.poll_write(), None);
        Ok(())
    }

    #[test]
    fn client_stream_opened_sends_client_setup_for_webtransport() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(true));

        protocol.handle_event(EventIn::StreamOpened {
            stream_id: 7,
            bidi: true,
            local: true,
        })?;

        let Some(WriteOutput::SendStream {
            stream_id,
            bytes,
            fin,
        }) = protocol.poll_write()
        else {
            panic!("expected setup bytes");
        };
        assert_eq!(stream_id, 7);
        assert!(fin);

        let mut parser = MessageParser::new(true);
        parser.process_data(&mut bytes.as_ref(), true);
        let event = parser.poll_event().expect("control event");
        match event {
            crate::message::message_parser::MessageParserEvent::ControlMessage(
                ControlMessage::ClientSetup(client_setup),
            ) => {
                assert_eq!(client_setup.supported_versions, vec![Version::Draft04]);
                assert_eq!(client_setup.role, Some(Role::PubSub));
                assert_eq!(client_setup.path, None);
                assert!(client_setup.uses_web_transport);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn client_stream_opened_sends_client_setup_for_raw_quic() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));

        protocol.handle_event(EventIn::StreamOpened {
            stream_id: 9,
            bidi: true,
            local: true,
        })?;

        let Some(WriteOutput::SendStream { bytes, .. }) = protocol.poll_write() else {
            panic!("expected setup bytes");
        };

        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), true);
        let event = parser.poll_event().expect("control event");
        match event {
            crate::message::message_parser::MessageParserEvent::ControlMessage(
                ControlMessage::ClientSetup(client_setup),
            ) => {
                assert_eq!(client_setup.path, Some("/moq".to_string()));
                assert!(!client_setup.uses_web_transport);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }
}
