use bytes::{Bytes, BytesMut};
use moqt::{
    ClientSetup, Command, Connection, EventIn, EventOut, FilterType, FullSequence,
    FullTrackName, ObjectForwardingPreference, ProtocolConfig, ProtocolPerspective, Role,
    Serializer, ServerSetup, Session, SessionConfig, SessionCore, SessionDriver,
    SessionPerspective, SessionTransport, StreamPurpose, Subscribe, SubscribeOk, Version,
    WriteOutput,
};
use sansio::Protocol;
use std::time::Instant;

#[derive(Default)]
struct FakeTransport {
    next_stream_id: u32,
    opened_streams: Vec<(StreamPurpose, u32)>,
    sent_streams: Vec<(u32, BytesMut, bool)>,
    sent_datagrams: Vec<Bytes>,
    closes: Vec<(u64, String)>,
}

impl FakeTransport {
    fn new(next_stream_id: u32) -> Self {
        Self {
            next_stream_id,
            ..Self::default()
        }
    }
}

impl SessionTransport for FakeTransport {
    fn open_bi_stream(&mut self, purpose: StreamPurpose) -> moqt::Result<u32> {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 2;
        self.opened_streams.push((purpose, stream_id));
        Ok(stream_id)
    }

    fn send_stream(&mut self, stream_id: u32, bytes: BytesMut, fin: bool) -> moqt::Result<()> {
        self.sent_streams.push((stream_id, bytes, fin));
        Ok(())
    }

    fn send_datagram(&mut self, bytes: Bytes) -> moqt::Result<()> {
        self.sent_datagrams.push(bytes);
        Ok(())
    }

    fn close(&mut self, code: u64, reason: String) -> moqt::Result<()> {
        self.closes.push((code, reason));
        Ok(())
    }
}

fn client_protocol_config() -> ProtocolConfig {
    ProtocolConfig {
        version: Version::Draft04,
        perspective: ProtocolPerspective::Client,
        use_web_transport: false,
        path: "/moq".to_string(),
        deliver_partial_objects: false,
    }
}

fn client_session_config() -> SessionConfig {
    SessionConfig {
        version: Version::Draft04,
        perspective: SessionPerspective::Client,
        use_web_transport: false,
        path: "/moq".to_string(),
        deliver_partial_objects: false,
    }
}

fn server_session_config() -> SessionConfig {
    SessionConfig {
        version: Version::Draft04,
        perspective: SessionPerspective::Server,
        use_web_transport: false,
        path: "/moq".to_string(),
        deliver_partial_objects: false,
    }
}

fn encode_control<T: Serializer>(message_type: u64, message: &T) -> moqt::Result<Bytes> {
    let mut buf = Vec::new();
    let _ = message_type.serialize(&mut buf)?;
    let _ = message.serialize(&mut buf)?;
    Ok(Bytes::from(buf))
}

#[test]
fn public_session_core_smoke_test() -> moqt::Result<()> {
    let mut core = SessionCore::new(client_protocol_config());

    core.handle_event(EventIn::TransportConnected)?;
    assert_eq!(
        core.poll_write(),
        Some(WriteOutput::OpenBiStream {
            purpose: StreamPurpose::Control,
        })
    );

    core.handle_event(EventIn::StreamOpened {
        stream_id: 7,
        bidi: true,
        local: true,
    })?;

    match core.poll_write() {
        Some(WriteOutput::SendStream {
            stream_id,
            bytes,
            fin,
        }) => {
            assert_eq!(stream_id, 7);
            assert!(!bytes.is_empty());
            assert!(!fin);
        }
        other => panic!("unexpected write output: {other:?}"),
    }

    Ok(())
}

#[test]
fn public_session_driver_smoke_test() -> moqt::Result<()> {
    let transport = FakeTransport::new(11);
    let mut driver = SessionDriver::new(client_protocol_config(), transport);

    driver.on_transport_connected()?;

    assert_eq!(
        driver.transport().opened_streams,
        vec![(StreamPurpose::Control, 11)]
    );
    assert_eq!(driver.transport().sent_streams.len(), 1);
    assert!(driver.transport().sent_streams[0].1.len() > 0);
    assert!(driver.poll_event().is_none());

    Ok(())
}

#[test]
fn public_session_wrapper_smoke_test() -> moqt::Result<()> {
    let mut session = Session::new(client_session_config(), Connection::QUIC);

    session.on_transport_connected()?;
    session.handle_timeout(Instant::now())?;

    assert_eq!(session.poll_event(), None::<EventOut>);

    session.handle_command(Command::RegisterLocalTrack {
        track_namespace: "live".to_string(),
        track_name: "camera".to_string(),
        forwarding_preference: ObjectForwardingPreference::Datagram,
        next_sequence: None,
    })?;

    let _transport = session.into_transport();
    Ok(())
}

#[test]
fn public_session_external_subscribe_round_trip() -> moqt::Result<()> {
    let mut session = Session::new(client_session_config(), Connection::QUIC);

    session.on_transport_connected()?;
    session.on_stream_data(
        0,
        encode_control(
            0x41,
            &ServerSetup {
                supported_version: Version::Draft04,
                role: Some(Role::PubSub),
            },
        )?,
        false,
    )?;
    let _ = session.poll_event();

    session.handle_command(Command::Subscribe {
        track_namespace: "live".to_string(),
        track_name: "camera".to_string(),
        filter_type: FilterType::LatestObject,
        authorization_info: None,
    })?;

    session.on_stream_data(
        0,
        encode_control(
            0x04,
            &SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: Some(FullSequence::new(7, 2)),
            },
        )?,
        false,
    )?;

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
fn public_session_external_server_receives_subscribe() -> moqt::Result<()> {
    let mut session = Session::new(server_session_config(), Connection::QUIC);

    session.handle_command(Command::RegisterLocalTrack {
        track_namespace: "live".to_string(),
        track_name: "camera".to_string(),
        forwarding_preference: ObjectForwardingPreference::Datagram,
        next_sequence: None,
    })?;

    session.on_stream_data(
        0,
        encode_control(
            0x40,
            &ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            },
        )?,
        false,
    )?;
    let _ = session.poll_event();

    let subscribe = Subscribe {
        subscribe_id: 7,
        track_alias: 9,
        track_namespace: "live".to_string(),
        track_name: "camera".to_string(),
        filter_type: FilterType::AbsoluteStart(FullSequence::new(0, 0)),
        authorization_info: None,
    };
    session.on_stream_data(0, encode_control(0x03, &subscribe)?, false)?;

    assert_eq!(session.poll_event(), Some(EventOut::SubscribeReceived(subscribe)));
    Ok(())
}
