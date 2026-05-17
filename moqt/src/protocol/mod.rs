use crate::message::client_setup::ClientSetup;
use crate::message::message_framer::MessageFramer;
use crate::message::message_parser::{MessageParser, MessageParserEvent};
use crate::message::server_setup::ServerSetup;
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
    SessionEstablished {
        peer_role: Option<Role>,
        path: Option<String>,
    },
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
    control_parser: Option<MessageParser>,
    routs: VecDeque<ReadOutput>,
    wouts: VecDeque<WriteOutput>,
    eouts: VecDeque<EventOut>,
}

impl SessionCore {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            control_stream_id: None,
            control_parser: None,
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

    fn send_server_setup(&mut self, stream_id: StreamId) -> Result<()> {
        let server_setup = ServerSetup {
            supported_version: self.config.version,
            role: Some(Role::PubSub),
        };

        let mut bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ServerSetup(server_setup),
            &mut bytes,
        )?;
        self.wouts.push_back(WriteOutput::SendStream {
            stream_id,
            bytes,
            fin: true,
        });
        Ok(())
    }

    fn ensure_control_stream(&mut self, stream_id: StreamId) {
        if self.control_stream_id.is_none() {
            self.control_stream_id = Some(stream_id);
        }
        if self.control_parser.is_none() {
            self.control_parser = Some(MessageParser::new(self.config.use_web_transport));
        }
    }

    fn on_control_message(&mut self, control_message: ControlMessage) -> Result<()> {
        match control_message {
            ControlMessage::ClientSetup(client_setup) => {
                if self.config.perspective != Perspective::Server {
                    self.wouts.push_back(WriteOutput::Close {
                        code: 1,
                        reason: "received CLIENT_SETUP as client".to_string(),
                    });
                    return Ok(());
                }
                if !client_setup
                    .supported_versions
                    .contains(&self.config.version)
                {
                    self.wouts.push_back(WriteOutput::Close {
                        code: 1,
                        reason: format!("version mismatch: expected {:?}", self.config.version),
                    });
                    return Ok(());
                }
                let stream_id = self.control_stream_id.expect("control stream set");
                self.send_server_setup(stream_id)?;
                self.eouts.push_back(EventOut::SessionEstablished {
                    peer_role: client_setup.role,
                    path: client_setup.path,
                });
            }
            ControlMessage::ServerSetup(server_setup) => {
                if self.config.perspective != Perspective::Client {
                    self.wouts.push_back(WriteOutput::Close {
                        code: 1,
                        reason: "received SERVER_SETUP as server".to_string(),
                    });
                    return Ok(());
                }
                if server_setup.supported_version != self.config.version {
                    self.wouts.push_back(WriteOutput::Close {
                        code: 1,
                        reason: format!("version mismatch: expected {:?}", self.config.version),
                    });
                    return Ok(());
                }
                self.eouts.push_back(EventOut::SessionEstablished {
                    peer_role: server_setup.role,
                    path: None,
                });
            }
            _ => {
                self.routs
                    .push_back(ReadOutput::Datagram(Bytes::from_static(
                        b"unhandled-control-message",
                    )));
            }
        }
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
        match msg {
            ReadInput::StreamData {
                stream_id,
                data,
                fin,
            } => {
                self.ensure_control_stream(stream_id);
                if self.control_stream_id == Some(stream_id) {
                    let mut events = Vec::new();
                    let parser = self.control_parser.as_mut().expect("control parser set");
                    parser.process_data(&mut data.as_ref(), fin);
                    while let Some(event) = parser.poll_event() {
                        events.push(event);
                    }
                    for event in events {
                        match event {
                            MessageParserEvent::ControlMessage(control_message) => {
                                self.on_control_message(control_message)?;
                            }
                            MessageParserEvent::ParsingError(_, reason) => {
                                self.wouts.push_back(WriteOutput::Close { code: 1, reason });
                            }
                            MessageParserEvent::ObjectMessage(_, _, _) => {
                                self.wouts.push_back(WriteOutput::Close {
                                    code: 1,
                                    reason: "received object on control stream".to_string(),
                                });
                            }
                        }
                    }
                } else {
                    self.routs.push_back(ReadOutput::StreamData {
                        stream_id,
                        data,
                        fin,
                    });
                }
            }
            ReadInput::Datagram(bytes) => self.routs.push_back(ReadOutput::Datagram(bytes)),
        }
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
                self.control_parser = None;
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
                    self.ensure_control_stream(stream_id);
                    self.send_client_setup(stream_id)?;
                }
            }
            EventIn::StreamClosed { stream_id } => {
                if self.control_stream_id == Some(stream_id) {
                    self.control_stream_id = None;
                    self.control_parser = None;
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

    fn server_config(use_web_transport: bool) -> Config {
        Config {
            version: Version::Draft04,
            perspective: Perspective::Server,
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

    #[test]
    fn server_receives_client_setup_and_replies_with_server_setup() -> Result<()> {
        let mut client_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            }),
            &mut client_setup_bytes,
        )?;

        let mut protocol = SessionCore::new(server_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 11,
            data: client_setup_bytes.freeze(),
            fin: true,
        })?;

        let Some(WriteOutput::SendStream {
            stream_id, bytes, ..
        }) = protocol.poll_write()
        else {
            panic!("expected server setup response");
        };
        assert_eq!(stream_id, 11);

        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), true);
        let event = parser.poll_event().expect("control event");
        match event {
            MessageParserEvent::ControlMessage(ControlMessage::ServerSetup(server_setup)) => {
                assert_eq!(server_setup.supported_version, Version::Draft04);
                assert_eq!(server_setup.role, Some(Role::PubSub));
            }
            _ => panic!("unexpected parser event"),
        }

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SessionEstablished {
                peer_role: Some(Role::PubSub),
                path: Some("/moq".to_string())
            })
        );
        Ok(())
    }

    #[test]
    fn client_receives_server_setup_and_establishes_session() -> Result<()> {
        let mut server_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ServerSetup(ServerSetup {
                supported_version: Version::Draft04,
                role: Some(Role::PubSub),
            }),
            &mut server_setup_bytes,
        )?;

        let mut protocol = SessionCore::new(client_config(true));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 13,
            data: server_setup_bytes.freeze(),
            fin: true,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SessionEstablished {
                peer_role: Some(Role::PubSub),
                path: None
            })
        );
        Ok(())
    }
}
