use bytes::{Bytes, BytesMut};
use moqt::{
    ClientSetup, Command, Connection, ControlMessage, EventIn, EventOut, FilterType, FullSequence,
    FullTrackName, MessageFramer, MessageParser, MessageParserEvent, ObjectForwardingPreference,
    ObjectHeader, ObjectStatus, ProtocolConfig, ProtocolPerspective, RemoteTrackOnObjectFragment,
    Role, ServerSetup, Session, SessionConfig, SessionCore, SessionDriver, SessionPerspective,
    SessionTransport, StreamId, StreamPurpose, Subscribe, SubscribeDone, SubscribeError,
    SubscribeOk, Version, WriteOutput,
};
use sansio::Protocol;
use std::time::Instant;

#[derive(Default)]
struct FakeTransport {
    next_stream_id: StreamId,
    opened_streams: Vec<(StreamPurpose, StreamId)>,
    sent_streams: Vec<(StreamId, BytesMut, bool)>,
    sent_datagrams: Vec<Bytes>,
    closes: Vec<(u64, String)>,
}

impl FakeTransport {
    fn new(next_stream_id: StreamId) -> Self {
        Self {
            next_stream_id,
            ..Self::default()
        }
    }
}

impl SessionTransport for FakeTransport {
    fn open_bi_stream(&mut self, purpose: StreamPurpose) -> moqt::Result<StreamId> {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 2;
        self.opened_streams.push((purpose, stream_id));
        Ok(stream_id)
    }

    fn send_stream(&mut self, stream_id: StreamId, bytes: BytesMut, fin: bool) -> moqt::Result<()> {
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

fn server_protocol_config() -> ProtocolConfig {
    ProtocolConfig {
        version: Version::Draft04,
        perspective: ProtocolPerspective::Server,
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

fn encode_control(message: ControlMessage) -> moqt::Result<Bytes> {
    let mut buf = BytesMut::new();
    let _ = MessageFramer::serialize_control_message(message, &mut buf)?;
    Ok(buf.freeze())
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
fn public_session_driver_surfaces_incoming_subscribe() -> moqt::Result<()> {
    let transport = FakeTransport::new(10);
    let mut driver = SessionDriver::new(server_protocol_config(), transport);

    driver.on_stream_data(
        0,
        encode_control(ControlMessage::ClientSetup(ClientSetup {
            supported_versions: vec![Version::Draft04],
            role: Some(Role::PubSub),
            path: Some("/moq".to_string()),
            uses_web_transport: false,
        }))?,
        false,
    )?;
    let _ = driver.poll_event();

    let subscribe = Subscribe {
        subscribe_id: 7,
        track_alias: 9,
        track_namespace: "live".to_string(),
        track_name: "camera".to_string(),
        filter_type: FilterType::AbsoluteStart(FullSequence::new(0, 0)),
        authorization_info: None,
    };

    driver.on_stream_data(
        0,
        encode_control(ControlMessage::Subscribe(subscribe.clone()))?,
        false,
    )?;

    assert_eq!(
        driver.poll_event(),
        Some(EventOut::SubscribeReceived(subscribe))
    );

    Ok(())
}

#[test]
fn public_session_driver_publishes_object_datagram() -> moqt::Result<()> {
    let transport = FakeTransport::new(10);
    let mut driver = SessionDriver::new(server_protocol_config(), transport);

    driver.on_stream_data(
        0,
        encode_control(ControlMessage::ClientSetup(ClientSetup {
            supported_versions: vec![Version::Draft04],
            role: Some(Role::PubSub),
            path: Some("/moq".to_string()),
            uses_web_transport: false,
        }))?,
        false,
    )?;
    let _ = driver.transport_mut().sent_streams.pop();
    let _ = driver.poll_event();

    driver.handle_command(Command::RegisterLocalTrack {
        track_namespace: "live".to_string(),
        track_name: "camera".to_string(),
        forwarding_preference: ObjectForwardingPreference::Datagram,
        next_sequence: None,
    })?;

    driver.on_stream_data(
        0,
        encode_control(ControlMessage::Subscribe(Subscribe {
            subscribe_id: 7,
            track_alias: 9,
            track_namespace: "live".to_string(),
            track_name: "camera".to_string(),
            filter_type: FilterType::AbsoluteStart(FullSequence::new(0, 0)),
            authorization_info: None,
        }))?,
        false,
    )?;
    let _ = driver.poll_event();

    driver.handle_command(Command::SubscribeOk {
        subscribe_id: 7,
        expires: 60,
        largest_group_object: None,
    })?;
    let _ = driver.transport_mut().sent_streams.pop();

    driver.handle_command(Command::PublishObject {
        track_namespace: "live".to_string(),
        track_name: "camera".to_string(),
        group_id: 1,
        object_id: 2,
        send_order: 3,
        status: ObjectStatus::Normal,
        payload: Bytes::from_static(b"frame"),
    })?;

    assert!(driver.transport().opened_streams.is_empty());
    assert_eq!(driver.transport().sent_datagrams.len(), 1);

    let (header, payload) =
        MessageParser::process_datagram(&mut driver.transport().sent_datagrams[0].as_ref())?;
    assert_eq!(header.subscribe_id, 7);
    assert_eq!(header.track_alias, 9);
    assert_eq!(header.group_id, 1);
    assert_eq!(header.object_id, 2);
    assert_eq!(header.object_send_order, 3);
    assert_eq!(header.object_status, ObjectStatus::Normal);
    assert_eq!(
        header.object_forwarding_preference,
        ObjectForwardingPreference::Datagram
    );
    assert_eq!(payload, Bytes::from_static(b"frame"));

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
        encode_control(ControlMessage::ServerSetup(ServerSetup {
            supported_version: Version::Draft04,
            role: Some(Role::PubSub),
        }))?,
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
        encode_control(ControlMessage::SubscribeOk(SubscribeOk {
            subscribe_id: 0,
            expires: 30,
            largest_group_object: Some(FullSequence::new(7, 2)),
        }))?,
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
fn public_session_external_subscribe_rejection_round_trip() -> moqt::Result<()> {
    let mut session = Session::new(client_session_config(), Connection::QUIC);

    session.on_transport_connected()?;
    session.on_stream_data(
        0,
        encode_control(ControlMessage::ServerSetup(ServerSetup {
            supported_version: Version::Draft04,
            role: Some(Role::PubSub),
        }))?,
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
        encode_control(ControlMessage::SubscribeError(SubscribeError {
            subscribe_id: 0,
            error_code: 403,
            reason_phrase: "forbidden".to_string(),
            track_alias: 0,
        }))?,
        false,
    )?;

    assert_eq!(
        session.poll_event(),
        Some(EventOut::SubscribeRejected {
            subscribe_id: 0,
            full_track_name: FullTrackName::new("live".to_string(), "camera".to_string()),
            error_code: 403,
            reason_phrase: "forbidden".to_string(),
            track_alias: 0,
        })
    );
    Ok(())
}

#[test]
fn public_session_external_subscribe_done_round_trip() -> moqt::Result<()> {
    let mut session = Session::new(client_session_config(), Connection::QUIC);

    session.on_transport_connected()?;
    session.on_stream_data(
        0,
        encode_control(ControlMessage::ServerSetup(ServerSetup {
            supported_version: Version::Draft04,
            role: Some(Role::PubSub),
        }))?,
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
        encode_control(ControlMessage::SubscribeOk(SubscribeOk {
            subscribe_id: 0,
            expires: 30,
            largest_group_object: None,
        }))?,
        false,
    )?;
    let _ = session.poll_event();

    session.on_stream_data(
        0,
        encode_control(ControlMessage::SubscribeDone(SubscribeDone {
            subscribe_id: 0,
            status_code: 206,
            reason_phrase: "finished".to_string(),
            final_group_object: Some(FullSequence::new(7, 2)),
        }))?,
        false,
    )?;

    assert_eq!(
        session.poll_event(),
        Some(EventOut::SubscribeEnded {
            subscribe_id: 0,
            full_track_name: FullTrackName::new("live".to_string(), "camera".to_string()),
            track_alias: 0,
            status_code: 206,
            reason_phrase: "finished".to_string(),
            final_group_object: Some(FullSequence::new(7, 2)),
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
        encode_control(ControlMessage::ClientSetup(ClientSetup {
            supported_versions: vec![Version::Draft04],
            role: Some(Role::PubSub),
            path: Some("/moq".to_string()),
            uses_web_transport: false,
        }))?,
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
    session.on_stream_data(
        0,
        encode_control(ControlMessage::Subscribe(subscribe.clone()))?,
        false,
    )?;

    assert_eq!(
        session.poll_event(),
        Some(EventOut::SubscribeReceived(subscribe))
    );
    Ok(())
}

#[test]
fn public_session_external_receives_object_datagram() -> moqt::Result<()> {
    let mut session = Session::new(client_session_config(), Connection::QUIC);

    session.on_transport_connected()?;
    session.on_stream_data(
        0,
        encode_control(ControlMessage::ServerSetup(ServerSetup {
            supported_version: Version::Draft04,
            role: Some(Role::PubSub),
        }))?,
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
        encode_control(ControlMessage::SubscribeOk(SubscribeOk {
            subscribe_id: 0,
            expires: 30,
            largest_group_object: None,
        }))?,
        false,
    )?;
    let _ = session.poll_event();

    let object_header = ObjectHeader {
        subscribe_id: 0,
        track_alias: 0,
        group_id: 3,
        object_id: 4,
        object_send_order: 0,
        object_status: ObjectStatus::Normal,
        object_forwarding_preference: ObjectForwardingPreference::Datagram,
        object_payload_length: None,
    };
    let mut datagram = BytesMut::new();
    MessageFramer::serialize_object_datagram(
        object_header,
        Bytes::from_static(b"xyz"),
        &mut datagram,
    )?;

    session.on_datagram(datagram.freeze())?;

    assert_eq!(
        session.poll_event(),
        Some(EventOut::ObjectReceived {
            full_track_name: FullTrackName::new("live".to_string(), "camera".to_string()),
            fragment: RemoteTrackOnObjectFragment {
                object_header,
                payload: Bytes::from_static(b"xyz"),
                fin: true,
            },
        })
    );
    Ok(())
}

#[test]
fn public_wire_helpers_round_trip_object_stream() -> moqt::Result<()> {
    let mut bytes = BytesMut::new();
    MessageFramer::serialize_object(
        ObjectHeader {
            subscribe_id: 7,
            track_alias: 9,
            group_id: 1,
            object_id: 2,
            object_send_order: 3,
            object_status: ObjectStatus::Normal,
            object_forwarding_preference: ObjectForwardingPreference::Object,
            object_payload_length: None,
        },
        true,
        Bytes::from_static(b"frame"),
        &mut bytes,
    )?;

    let mut parser = MessageParser::new(false);
    parser.process_data(&mut bytes.freeze().as_ref(), true);

    match parser.poll_event() {
        Some(MessageParserEvent::ObjectMessage(header, payload, fin)) => {
            assert_eq!(header.subscribe_id, 7);
            assert_eq!(header.track_alias, 9);
            assert_eq!(header.group_id, 1);
            assert_eq!(header.object_id, 2);
            assert_eq!(header.object_send_order, 3);
            assert_eq!(header.object_status, ObjectStatus::Normal);
            assert_eq!(
                header.object_forwarding_preference,
                ObjectForwardingPreference::Object
            );
            assert_eq!(payload, Bytes::from_static(b"frame"));
            assert!(fin);
        }
        other => panic!("unexpected parser event: {other:?}"),
    }

    Ok(())
}

#[test]
fn public_wire_helpers_round_trip_object_datagram() -> moqt::Result<()> {
    let mut bytes = BytesMut::new();
    MessageFramer::serialize_object_datagram(
        ObjectHeader {
            subscribe_id: 7,
            track_alias: 9,
            group_id: 1,
            object_id: 2,
            object_send_order: 3,
            object_status: ObjectStatus::Normal,
            object_forwarding_preference: ObjectForwardingPreference::Datagram,
            object_payload_length: None,
        },
        Bytes::from_static(b"frame"),
        &mut bytes,
    )?;

    let (header, payload) = MessageParser::process_datagram(&mut bytes.freeze().as_ref())?;
    assert_eq!(header.subscribe_id, 7);
    assert_eq!(header.track_alias, 9);
    assert_eq!(header.group_id, 1);
    assert_eq!(header.object_id, 2);
    assert_eq!(header.object_send_order, 3);
    assert_eq!(header.object_status, ObjectStatus::Normal);
    assert_eq!(
        header.object_forwarding_preference,
        ObjectForwardingPreference::Datagram
    );
    assert_eq!(payload, Bytes::from_static(b"frame"));

    Ok(())
}

#[test]
fn public_wire_helpers_round_trip_control_message() -> moqt::Result<()> {
    let mut bytes = BytesMut::new();
    MessageFramer::serialize_control_message(
        ControlMessage::ClientSetup(ClientSetup {
            supported_versions: vec![Version::Draft04],
            role: Some(Role::PubSub),
            path: Some("/moq".to_string()),
            uses_web_transport: false,
        }),
        &mut bytes,
    )?;

    let mut parser = MessageParser::new(false);
    parser.process_data(&mut bytes.freeze().as_ref(), false);

    match parser.poll_event() {
        Some(MessageParserEvent::ControlMessage(ControlMessage::ClientSetup(setup))) => {
            assert_eq!(setup.supported_versions, vec![Version::Draft04]);
            assert_eq!(setup.role, Some(Role::PubSub));
            assert_eq!(setup.path, Some("/moq".to_string()));
            assert!(!setup.uses_web_transport);
        }
        other => panic!("unexpected parser event: {other:?}"),
    }

    Ok(())
}
