use crate::connection::Connection;
use crate::driver::SessionDriver;
use crate::protocol::{self, Command, EventOut};
use crate::Result;
use bytes::Bytes;
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

pub struct Session {
    driver: SessionDriver<Connection>,
}

impl Session {
    pub fn new(config: config::Config, conn: Connection) -> Self {
        Self {
            driver: SessionDriver::new(config.into(), conn),
        }
    }

    pub fn transport(&self) -> &Connection {
        self.driver.transport()
    }

    pub fn transport_mut(&mut self) -> &mut Connection {
        self.driver.transport_mut()
    }

    pub fn into_transport(self) -> Connection {
        self.driver.into_transport()
    }

    pub fn on_transport_connected(&mut self) -> Result<()> {
        self.driver.on_transport_connected()
    }

    pub fn on_transport_closed(&mut self) -> Result<()> {
        self.driver.on_transport_closed()
    }

    pub fn on_stream_opened(&mut self, stream_id: u32, bidi: bool, local: bool) -> Result<()> {
        self.driver.on_stream_opened(stream_id, bidi, local)
    }

    pub fn on_stream_closed(&mut self, stream_id: u32) -> Result<()> {
        self.driver.on_stream_closed(stream_id)
    }

    pub fn on_stream_data(&mut self, stream_id: u32, data: Bytes, fin: bool) -> Result<()> {
        self.driver.on_stream_data(stream_id, data, fin)
    }

    pub fn on_datagram(&mut self, bytes: Bytes) -> Result<()> {
        self.driver.on_datagram(bytes)
    }

    pub fn handle_command(&mut self, command: Command) -> Result<()> {
        self.driver.handle_command(command)
    }

    pub fn poll_event(&mut self) -> Option<EventOut> {
        self.driver.poll_event()
    }

    pub fn handle_timeout(&mut self, now: Instant) -> Result<()> {
        self.driver.handle_timeout(now)
    }

    pub fn poll_timeout(&mut self) -> Option<Instant> {
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

        session.on_transport_connected()?;

        let mut server_setup_bytes = bytes::BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ServerSetup(ServerSetup {
                supported_version: Version::Draft04,
                role: Some(Role::PubSub),
            }),
            &mut server_setup_bytes,
        )?;

        session.on_stream_data(0, server_setup_bytes.freeze(), false)?;

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

        session.on_transport_closed()?;

        assert_eq!(session.poll_event(), Some(EventOut::SessionTerminated));
        Ok(())
    }

    #[test]
    fn session_wrapper_handles_outgoing_subscribe_round_trip() -> Result<()> {
        let mut session = Session::new(client_config(), Connection::QUIC);

        session.on_transport_connected()?;

        let mut server_setup_bytes = bytes::BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ServerSetup(ServerSetup {
                supported_version: Version::Draft04,
                role: Some(Role::PubSub),
            }),
            &mut server_setup_bytes,
        )?;
        session.on_stream_data(0, server_setup_bytes.freeze(), false)?;
        let _ = session.poll_event();

        session.handle_command(Command::Subscribe {
            track_namespace: "live".to_string(),
            track_name: "camera".to_string(),
            filter_type: FilterType::LatestObject,
            authorization_info: None,
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
        session.on_stream_data(0, subscribe_ok_bytes.freeze(), false)?;

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

        session.handle_command(Command::RegisterLocalTrack {
            track_namespace: "live".to_string(),
            track_name: "camera".to_string(),
            forwarding_preference: ObjectForwardingPreference::Datagram,
            next_sequence: None,
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
        session.on_stream_data(0, client_setup_bytes.freeze(), false)?;
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
        session.on_stream_data(0, subscribe_bytes.freeze(), false)?;

        assert_eq!(
            session.poll_event(),
            Some(EventOut::SubscribeReceived(subscribe))
        );
        Ok(())
    }
}
