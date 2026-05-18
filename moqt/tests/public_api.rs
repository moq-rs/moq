use bytes::{Bytes, BytesMut};
use moqt::{
    Command, Connection, EventIn, EventOut, ObjectForwardingPreference, ProtocolConfig,
    ProtocolPerspective, Session, SessionConfig, SessionCore, SessionDriver, SessionPerspective,
    SessionTransport, StreamPurpose, Version, WriteOutput,
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
