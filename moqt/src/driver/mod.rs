use crate::protocol::{
    Command, Config, EventIn, EventOut, ReadInput, SessionCore, StreamPurpose, WriteOutput,
};
use crate::{Result, StreamId};
use bytes::{Bytes, BytesMut};
use sansio::Protocol;
use std::collections::VecDeque;

pub(crate) trait SessionTransport {
    fn open_bi_stream(&mut self, purpose: StreamPurpose) -> Result<StreamId>;
    fn send_stream(&mut self, stream_id: StreamId, bytes: BytesMut, fin: bool) -> Result<()>;
    fn send_datagram(&mut self, bytes: Bytes) -> Result<()>;
    fn close(&mut self, code: u64, reason: String) -> Result<()>;
}

pub(crate) struct SessionDriver<T> {
    protocol: SessionCore,
    transport: T,
    events: VecDeque<EventOut>,
}

impl<T: SessionTransport> SessionDriver<T> {
    pub(crate) fn new(config: Config, transport: T) -> Self {
        Self {
            protocol: SessionCore::new(config),
            transport,
            events: VecDeque::new(),
        }
    }

    pub(crate) fn transport(&self) -> &T {
        &self.transport
    }

    pub(crate) fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub(crate) fn on_transport_connected(&mut self) -> Result<()> {
        self.protocol.handle_event(EventIn::TransportConnected)?;
        self.flush()
    }

    pub(crate) fn on_transport_closed(&mut self) -> Result<()> {
        self.protocol.handle_event(EventIn::TransportClosed)?;
        self.flush()
    }

    pub(crate) fn on_stream_opened(
        &mut self,
        stream_id: StreamId,
        bidi: bool,
        local: bool,
    ) -> Result<()> {
        self.protocol.handle_event(EventIn::StreamOpened {
            stream_id,
            bidi,
            local,
        })?;
        self.flush()
    }

    pub(crate) fn on_stream_closed(&mut self, stream_id: StreamId) -> Result<()> {
        self.protocol
            .handle_event(EventIn::StreamClosed { stream_id })?;
        self.flush()
    }

    pub(crate) fn on_stream_data(
        &mut self,
        stream_id: StreamId,
        data: Bytes,
        fin: bool,
    ) -> Result<()> {
        self.protocol.handle_read(ReadInput::StreamData {
            stream_id,
            data,
            fin,
        })?;
        self.flush()
    }

    pub(crate) fn on_datagram(&mut self, bytes: Bytes) -> Result<()> {
        self.protocol.handle_read(ReadInput::Datagram(bytes))?;
        self.flush()
    }

    pub(crate) fn handle_command(&mut self, command: Command) -> Result<()> {
        self.protocol.handle_write(command)?;
        self.flush()
    }

    pub(crate) fn poll_event(&mut self) -> Option<EventOut> {
        self.events.pop_front()
    }

    fn flush(&mut self) -> Result<()> {
        loop {
            let mut progressed = false;

            while let Some(write) = self.protocol.poll_write() {
                progressed = true;
                match write {
                    WriteOutput::OpenBiStream { purpose } => {
                        let stream_id = self.transport.open_bi_stream(purpose)?;
                        self.protocol.handle_event(EventIn::StreamOpened {
                            stream_id,
                            bidi: true,
                            local: true,
                        })?;
                    }
                    WriteOutput::SendStream {
                        stream_id,
                        bytes,
                        fin,
                    } => self.transport.send_stream(stream_id, bytes, fin)?,
                    WriteOutput::SendDatagram(bytes) => self.transport.send_datagram(bytes)?,
                    WriteOutput::Close { code, reason } => self.transport.close(code, reason)?,
                }
            }

            while let Some(event) = self.protocol.poll_event() {
                progressed = true;
                self.events.push_back(event);
            }

            if !progressed {
                break;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::client_setup::ClientSetup;
    use crate::message::message_framer::MessageFramer;
    use crate::message::message_parser::{MessageParser, MessageParserEvent};
    use crate::message::object::{ObjectForwardingPreference, ObjectStatus};
    use crate::message::subscribe::Subscribe;
    use crate::message::{ControlMessage, FilterType, FullSequence, Role, Version};

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
        fn open_bi_stream(&mut self, purpose: StreamPurpose) -> Result<StreamId> {
            let stream_id = self.next_stream_id;
            self.next_stream_id += 2;
            self.opened_streams.push((purpose, stream_id));
            Ok(stream_id)
        }

        fn send_stream(&mut self, stream_id: StreamId, bytes: BytesMut, fin: bool) -> Result<()> {
            self.sent_streams.push((stream_id, bytes, fin));
            Ok(())
        }

        fn send_datagram(&mut self, bytes: Bytes) -> Result<()> {
            self.sent_datagrams.push(bytes);
            Ok(())
        }

        fn close(&mut self, code: u64, reason: String) -> Result<()> {
            self.closes.push((code, reason));
            Ok(())
        }
    }

    fn client_config(use_web_transport: bool) -> Config {
        Config {
            version: Version::Draft04,
            perspective: crate::protocol::Perspective::Client,
            use_web_transport,
            path: "/moq".to_string(),
            deliver_partial_objects: false,
        }
    }

    fn server_config(use_web_transport: bool) -> Config {
        Config {
            version: Version::Draft04,
            perspective: crate::protocol::Perspective::Server,
            use_web_transport,
            path: "/moq".to_string(),
            deliver_partial_objects: false,
        }
    }

    #[test]
    fn client_driver_opens_control_stream_and_sends_setup() -> Result<()> {
        let transport = FakeTransport::new(7);
        let mut driver = SessionDriver::new(client_config(false), transport);

        driver.on_transport_connected()?;

        assert_eq!(
            driver.transport().opened_streams,
            vec![(StreamPurpose::Control, 7)]
        );
        assert_eq!(driver.transport().sent_streams.len(), 1);
        let (stream_id, bytes, fin) = &driver.transport().sent_streams[0];
        assert_eq!(*stream_id, 7);
        assert!(!*fin);

        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        match parser.poll_event() {
            Some(MessageParserEvent::ControlMessage(ControlMessage::ClientSetup(setup))) => {
                assert_eq!(setup.supported_versions, vec![Version::Draft04]);
                assert_eq!(setup.role, Some(Role::PubSub));
                assert_eq!(setup.path, Some("/moq".to_string()));
                assert!(!setup.uses_web_transport);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn server_driver_opens_data_stream_and_sends_track_object() -> Result<()> {
        let transport = FakeTransport::new(101);
        let mut driver = SessionDriver::new(server_config(false), transport);

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
        driver.on_stream_data(5, client_setup_bytes.freeze(), false)?;
        let _ = driver.transport_mut().sent_streams.pop();
        let _ = driver.poll_event();

        driver.handle_command(Command::RegisterLocalTrack {
            track_namespace: "live".to_string(),
            track_name: "camera".to_string(),
            forwarding_preference: ObjectForwardingPreference::Track,
            next_sequence: None,
        })?;

        let mut subscribe_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(Subscribe {
                subscribe_id: 7,
                track_alias: 9,
                track_namespace: "live".to_string(),
                track_name: "camera".to_string(),
                filter_type: FilterType::AbsoluteStart(FullSequence::new(0, 0)),
                authorization_info: None,
            }),
            &mut subscribe_bytes,
        )?;
        driver.on_stream_data(5, subscribe_bytes.freeze(), false)?;
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

        assert_eq!(
            driver.transport().opened_streams,
            vec![(StreamPurpose::Data, 101)]
        );
        assert_eq!(driver.transport().sent_streams.len(), 1);
        let (stream_id, bytes, fin) = &driver.transport().sent_streams[0];
        assert_eq!(*stream_id, 101);
        assert!(!*fin);

        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        match parser.poll_event() {
            Some(MessageParserEvent::ObjectMessage(header, payload, event_fin)) => {
                assert_eq!(header.subscribe_id, 7);
                assert_eq!(header.track_alias, 9);
                assert_eq!(header.group_id, 1);
                assert_eq!(header.object_id, 2);
                assert_eq!(
                    header.object_forwarding_preference,
                    ObjectForwardingPreference::Track
                );
                assert_eq!(payload, Bytes::from_static(b"frame"));
                assert!(event_fin);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn driver_surfaces_protocol_close_to_transport() -> Result<()> {
        let transport = FakeTransport::new(1);
        let mut driver = SessionDriver::new(client_config(false), transport);

        let mut subscribe_done_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeDone(crate::message::subscribe_done::SubscribeDone {
                subscribe_id: 77,
                status_code: 0,
                reason_phrase: "done".to_string(),
                final_group_object: None,
            }),
            &mut subscribe_done_bytes,
        )?;
        driver.on_stream_data(3, subscribe_done_bytes.freeze(), false)?;

        assert_eq!(
            driver.transport().closes,
            vec![(
                1,
                "received SUBSCRIBE_DONE before session setup".to_string()
            )]
        );
        Ok(())
    }
}
