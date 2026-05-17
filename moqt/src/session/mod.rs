use crate::connection::Connection;
use crate::driver::SessionDriver;
use crate::handler::Handler;
use crate::protocol::{self, Command, EventIn, EventOut, ReadInput, ReadOutput};
use crate::Result;
use retty::transport::Transmit;
use std::time::Instant;

pub mod config;
pub(crate) mod local_track;
pub(crate) mod remote_track;
mod subscribe_window;

impl From<config::Perspective> for protocol::Perspective {
    fn from(value: config::Perspective) -> Self {
        match value {
            config::Perspective::Server => Self::Server,
            config::Perspective::Client => Self::Client,
        }
    }
}

impl From<config::Config> for protocol::Config {
    fn from(value: config::Config) -> Self {
        Self {
            version: value.version,
            perspective: value.perspective.into(),
            use_web_transport: value.use_web_transport,
            path: value.path,
            deliver_partial_objects: value.deliver_partial_objects,
        }
    }
}

pub(crate) struct Session {
    driver: SessionDriver<Connection>,
}

impl Session {
    pub(crate) fn new(config: config::Config, conn: Connection) -> Self {
        Self {
            driver: SessionDriver::new(config.into(), conn),
        }
    }

    pub(crate) fn transport(&self) -> &Connection {
        self.driver.transport()
    }

    pub(crate) fn transport_mut(&mut self) -> &mut Connection {
        self.driver.transport_mut()
    }

    pub(crate) fn into_transport(self) -> Connection {
        self.driver.into_transport()
    }
}

impl Handler for Session {
    type Ein = EventIn;
    type Eout = EventOut;
    type Rin = ReadInput;
    type Rout = ReadOutput;
    type Win = Command;
    type Wout = ();

    fn transport_active(&mut self) -> Result<()> {
        self.driver.on_transport_connected()
    }

    fn transport_inactive(&mut self) -> Result<()> {
        self.driver.on_transport_closed()
    }

    fn handle_read(&mut self, msg: Transmit<Self::Rin>) -> Result<()> {
        match msg.message {
            ReadInput::StreamData {
                stream_id,
                data,
                fin,
            } => self.driver.on_stream_data(stream_id, data, fin),
            ReadInput::Datagram(bytes) => self.driver.on_datagram(bytes),
        }
    }

    fn poll_read(&mut self) -> Option<Transmit<Self::Rout>> {
        self.driver.poll_read().map(|message| Transmit {
            now: Instant::now(),
            transport: self.driver.transport().transport(),
            message,
        })
    }

    fn handle_write(&mut self, msg: Transmit<Self::Win>) -> Result<()> {
        self.driver.handle_command(msg.message)
    }

    fn poll_write(&mut self) -> Option<Transmit<Self::Wout>> {
        None
    }

    fn handle_event(&mut self, evt: Self::Ein) -> Result<()> {
        match evt {
            EventIn::TransportConnected => self.driver.on_transport_connected(),
            EventIn::TransportClosed => self.driver.on_transport_closed(),
            EventIn::StreamOpened {
                stream_id,
                bidi,
                local,
            } => self.driver.on_stream_opened(stream_id, bidi, local),
            EventIn::StreamClosed { stream_id } => self.driver.on_stream_closed(stream_id),
        }
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.driver.poll_event()
    }

    fn handle_timeout(&mut self, now: Instant) -> Result<()> {
        self.driver.handle_timeout(now)
    }

    fn poll_timeout(&mut self) -> Option<Instant> {
        self.driver.poll_timeout()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::client_setup::ClientSetup;
    use crate::message::message_framer::MessageFramer;
    use crate::message::object::ObjectForwardingPreference;
    use crate::message::server_setup::ServerSetup;
    use crate::message::subscribe::Subscribe;
    use crate::message::subscribe_ok::SubscribeOk;
    use crate::message::{ControlMessage, FilterType, FullSequence, FullTrackName, Role, Version};
    use retty::transport::{Transmit, TransportContext};

    fn client_config() -> config::Config {
        config::Config {
            version: Version::Draft04,
            perspective: config::Perspective::Client,
            use_web_transport: false,
            path: "/moq".to_string(),
            deliver_partial_objects: false,
        }
    }

    fn server_config() -> config::Config {
        config::Config {
            version: Version::Draft04,
            perspective: config::Perspective::Server,
            use_web_transport: false,
            path: "/moq".to_string(),
            deliver_partial_objects: false,
        }
    }

    #[test]
    fn session_wrapper_establishes_client_session_from_handler_surface() -> Result<()> {
        let mut session = Session::new(client_config(), Connection::QUIC);

        session.transport_active()?;

        let mut server_setup_bytes = bytes::BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ServerSetup(ServerSetup {
                supported_version: Version::Draft04,
                role: Some(Role::PubSub),
            }),
            &mut server_setup_bytes,
        )?;

        session.handle_read(Transmit {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: ReadInput::StreamData {
                stream_id: 0,
                data: server_setup_bytes.freeze(),
                fin: false,
            },
        })?;

        assert_eq!(
            session.poll_event(),
            Some(EventOut::SessionEstablished {
                peer_role: Some(Role::PubSub),
                path: None,
            })
        );
        Ok(())
    }

    #[test]
    fn session_wrapper_emits_termination_on_transport_inactive() -> Result<()> {
        let mut session = Session::new(client_config(), Connection::QUIC);

        session.transport_inactive()?;

        assert_eq!(session.poll_event(), Some(EventOut::SessionTerminated));
        Ok(())
    }

    #[test]
    fn session_wrapper_handles_outgoing_subscribe_round_trip() -> Result<()> {
        let mut session = Session::new(client_config(), Connection::QUIC);

        session.transport_active()?;

        let mut server_setup_bytes = bytes::BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ServerSetup(ServerSetup {
                supported_version: Version::Draft04,
                role: Some(Role::PubSub),
            }),
            &mut server_setup_bytes,
        )?;
        session.handle_read(Transmit {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: ReadInput::StreamData {
                stream_id: 0,
                data: server_setup_bytes.freeze(),
                fin: false,
            },
        })?;
        let _ = session.poll_event();

        session.handle_write(Transmit {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: Command::Subscribe {
                track_namespace: "live".to_string(),
                track_name: "camera".to_string(),
                filter_type: FilterType::LatestObject,
                authorization_info: None,
            },
        })?;

        let mut subscribe_ok_bytes = bytes::BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeOk(SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: Some(FullSequence::new(7, 2)),
            }),
            &mut subscribe_ok_bytes,
        )?;
        session.handle_read(Transmit {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: ReadInput::StreamData {
                stream_id: 0,
                data: subscribe_ok_bytes.freeze(),
                fin: false,
            },
        })?;

        assert_eq!(
            session.poll_event(),
            Some(EventOut::SubscribeAccepted {
                subscribe_id: 0,
                full_track_name: FullTrackName::new("live".to_string(), "camera".to_string()),
                track_alias: 0,
                expires: 30,
                largest_group_object: Some(FullSequence::new(7, 2)),
            })
        );
        Ok(())
    }

    #[test]
    fn session_wrapper_surfaces_incoming_subscribe_on_registered_track() -> Result<()> {
        let mut session = Session::new(server_config(), Connection::QUIC);

        session.handle_write(Transmit {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: Command::RegisterLocalTrack {
                track_namespace: "live".to_string(),
                track_name: "camera".to_string(),
                forwarding_preference: ObjectForwardingPreference::Datagram,
                next_sequence: None,
            },
        })?;

        let mut client_setup_bytes = bytes::BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            }),
            &mut client_setup_bytes,
        )?;
        session.handle_read(Transmit {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: ReadInput::StreamData {
                stream_id: 0,
                data: client_setup_bytes.freeze(),
                fin: false,
            },
        })?;
        let _ = session.poll_event();

        let subscribe = Subscribe {
            subscribe_id: 7,
            track_alias: 9,
            track_namespace: "live".to_string(),
            track_name: "camera".to_string(),
            filter_type: FilterType::AbsoluteStart(FullSequence::new(0, 0)),
            authorization_info: None,
        };
        let mut subscribe_bytes = bytes::BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(subscribe.clone()),
            &mut subscribe_bytes,
        )?;
        session.handle_read(Transmit {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: ReadInput::StreamData {
                stream_id: 0,
                data: subscribe_bytes.freeze(),
                fin: false,
            },
        })?;

        assert_eq!(
            session.poll_event(),
            Some(EventOut::SubscribeReceived(subscribe))
        );
        Ok(())
    }
}
