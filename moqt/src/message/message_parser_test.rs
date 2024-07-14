use crate::message::message_parser::{MessageParser, MessageParserEvent, ParserErrorCode};
use crate::message::message_test::{
    create_test_message, MessageStructuredData, TestMessageBase, TestObjectStreamMessage,
};
use crate::message::object::ObjectHeader;
use crate::message::{ControlMessage, MessageType};
use crate::Result;
use bytes::Bytes;
use rstest::rstest;
use std::fmt::{Display, Formatter};

struct TestParserParams {
    message_type: MessageType,
    uses_web_transport: bool,
}

impl Display for TestParserParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}_{}",
            self.message_type,
            if self.uses_web_transport {
                "WebTransport"
            } else {
                "QUIC"
            }
        )
    }
}

impl TestParserParams {
    fn new(message_type: MessageType, uses_web_transport: bool) -> Self {
        Self {
            message_type,
            uses_web_transport,
        }
    }
}

struct TestParserVisitor {
    object_payload: Option<Bytes>,
    end_of_message: bool,
    parsing_error: Option<String>,
    parsing_error_code: ParserErrorCode,
    messages_received: u64,
    last_message: Option<MessageStructuredData>,
}

impl TestParserVisitor {
    fn new() -> Self {
        Self {
            object_payload: None,
            end_of_message: false,
            parsing_error: None,
            parsing_error_code: ParserErrorCode::NoError,
            messages_received: 0,
            last_message: None,
        }
    }

    fn handle_event(&mut self, event: MessageParserEvent) {
        match event {
            MessageParserEvent::ParsingError(code, reason) => self.on_parsing_error(code, reason),
            MessageParserEvent::ObjectMessage(message, payload, end_of_message) => {
                self.on_object_message(message, payload, end_of_message)
            }
            MessageParserEvent::ControlMessage(message) => self.on_control_message(message),
        }
    }

    fn on_parsing_error(&mut self, code: ParserErrorCode, reason: String) {
        self.parsing_error = Some(reason);
        self.parsing_error_code = code;
    }

    fn on_object_message(&mut self, message: ObjectHeader, payload: Bytes, end_of_message: bool) {
        self.object_payload = Some(payload);
        self.end_of_message = end_of_message;
        self.messages_received += 1;
        self.last_message = Some(MessageStructuredData::Object(message));
    }

    fn on_control_message(&mut self, message: ControlMessage) {
        self.end_of_message = true;
        self.messages_received += 1;
        self.last_message = Some(MessageStructuredData::Control(message));
    }
}

struct TestParser {
    visitor: TestParserVisitor,
    message_type: MessageType,
    uses_web_transport: bool,
    parser: MessageParser,
}

impl TestParser {
    fn new(params: &TestParserParams) -> Self {
        Self {
            visitor: TestParserVisitor::new(),
            message_type: params.message_type,
            uses_web_transport: params.uses_web_transport,
            parser: MessageParser::new(params.uses_web_transport),
        }
    }

    fn make_message(&self) -> Box<dyn TestMessageBase> {
        create_test_message(self.message_type, self.uses_web_transport)
    }
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_parse_one_message(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    tester
        .parser
        .process_data(&mut message.packet_sample(), true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        1, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_message() {
        // Check payload message.
        assert!(
            tester.visitor.object_payload.is_some(),
            "message type {:?}",
            tester.message_type
        );
        assert_eq!(
            "foo",
            tester.visitor.object_payload.as_ref().unwrap(),
            "message type {:?}",
            tester.message_type
        );
    }

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_one_message_with_long_varints(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let mut message = tester.make_message();
    assert!(
        message.expand_varints().is_ok(),
        "message type {:?}",
        tester.message_type
    );
    tester
        .parser
        .process_data(&mut message.packet_sample(), true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        1, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_message() {
        // Check payload message.
        assert_eq!(
            "foo",
            tester.visitor.object_payload.as_ref().unwrap(),
            "message type {:?}",
            tester.message_type
        );
    }
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_two_part_message(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    // The test Object message has payload for less then half the message length,
    // so splitting the message in half will prevent the first half from being
    // processed.
    let mut first_data_size = total_message_size / 2;
    if tester.message_type == MessageType::StreamHeaderTrack {
        // The boundary happens to fall right after the stream header, so move it.
        first_data_size += 1;
    }
    tester
        .parser
        .process_data(&mut &message.packet_sample()[..first_data_size], false);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        0, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    tester.parser.process_data(
        &mut &message.packet_sample()[first_data_size..total_message_size],
        true,
    );
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(
        1, tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );

    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_message() {
        assert_eq!(
            "foo",
            tester.visitor.object_payload.as_ref().unwrap(),
            "message type {:?}",
            tester.message_type
        );
    }
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_one_byte_at_atime(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    let object_payload_size = 3;
    for i in 0..total_message_size {
        if !tester.message_type.is_object_message() {
            assert_eq!(
                0, tester.visitor.messages_received,
                "message type {:?} at {}-th byte of {} bytes",
                tester.message_type, i, total_message_size
            );
        }
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        tester
            .parser
            .process_data(&mut &message.packet_sample()[i..i + 1], false);
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
    }
    assert_eq!(
        if tester.message_type.is_object_message() {
            object_payload_size + 1
        } else {
            1
        },
        tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_without_payload_length() {
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        let empty: Vec<u8> = vec![];
        tester.parser.process_data(&mut &empty[..], true); // Needs the FIN
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
        assert_eq!(
            object_payload_size + 2,
            tester.visitor.messages_received,
            "message type {:?}",
            tester.message_type
        );
    }
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_one_byte_at_a_time_longer_varints(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let mut message = tester.make_message();
    assert!(
        message.expand_varints().is_ok(),
        "message type {:?}",
        tester.message_type
    );

    let total_message_size = message.packet_sample().len();
    let object_payload_size = 3;
    for i in 0..total_message_size {
        if !tester.message_type.is_object_message() {
            assert_eq!(
                0, tester.visitor.messages_received,
                "message type {:?}",
                tester.message_type
            );
        }
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        tester
            .parser
            .process_data(&mut &message.packet_sample()[i..i + 1], false);
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
    }
    assert_eq!(
        if tester.message_type.is_object_message() {
            object_payload_size + 1
        } else {
            1
        },
        tester.visitor.messages_received,
        "message type {:?}",
        tester.message_type
    );
    if tester.message_type.is_object_without_payload_length() {
        assert!(
            !tester.visitor.end_of_message,
            "message type {:?}",
            tester.message_type
        );
        let empty: Vec<u8> = vec![];
        tester.parser.process_data(&mut &empty[..], true); // Needs the FIN
        while let Some(event) = tester.parser.poll_event() {
            tester.visitor.handle_event(event);
        }
        assert_eq!(
            object_payload_size + 2,
            tester.visitor.messages_received,
            "message type {:?}",
            tester.message_type
        );
    }
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(
        message.equal_field_values(last_message),
        "message type {:?}",
        tester.message_type
    );
    assert!(
        tester.visitor.end_of_message,
        "message type {:?}",
        tester.message_type
    );
    assert!(
        !tester.visitor.parsing_error.is_some(),
        "message type {:?}",
        tester.message_type
    );

    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_early_fin(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    let mut first_data_size = total_message_size / 2;
    if tester.message_type == MessageType::StreamHeaderTrack {
        // The boundary happens to fall right after the stream header, so move it.
        first_data_size += 1;
    }
    tester
        .parser
        .process_data(&mut &message.packet_sample()[..first_data_size], true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(0, tester.visitor.messages_received);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("FIN after incomplete message".to_string())
    );
    Ok(())
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_separate_early_fin(params: (MessageType, bool)) -> Result<()> {
    let mut tester = TestParser::new(&TestParserParams::new(params.0, params.1));

    let message = tester.make_message();
    let total_message_size = message.packet_sample().len();
    let mut first_data_size = total_message_size / 2;
    if tester.message_type == MessageType::StreamHeaderTrack {
        // The boundary happens to fall right after the stream header, so move it.
        first_data_size += 1;
    }
    tester
        .parser
        .process_data(&mut &message.packet_sample()[..first_data_size], false);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    let empty: Vec<u8> = vec![];
    tester.parser.process_data(&mut &empty[..], true);
    while let Some(event) = tester.parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 0);
    assert!(tester.visitor.parsing_error.is_some());
    assert_eq!(
        tester.visitor.parsing_error,
        Some("End of stream before complete message".to_string())
    );
    assert_eq!(
        tester.visitor.parsing_error_code,
        ParserErrorCode::ProtocolViolation
    );
    Ok(())
}

const K_WEB_TRANS: bool = true;
const K_RAW_QUIC: bool = false;

// Tests for message-specific error cases, and behaviors for a single message
// type.
struct TestMessageSpecific {
    visitor: TestParserVisitor,
}

impl TestMessageSpecific {
    fn new() -> Self {
        Self {
            visitor: TestParserVisitor::new(),
        }
    }
}

#[test]
fn test_object_stream_separate_fin() -> Result<()> {
    let mut tester = TestMessageSpecific::new();
    // OBJECT can return on an unknown-length message even without receiving a
    // FIN.
    let mut parser = MessageParser::new(K_RAW_QUIC);
    let message = TestObjectStreamMessage::new();
    parser.process_data(&mut message.packet_sample(), false);
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 1);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(
        tester.visitor.object_payload,
        Some(Bytes::from_static(b"foo"))
    );
    assert!(!tester.visitor.end_of_message);

    let empty: Vec<u8> = vec![];
    parser.process_data(&mut &empty[..], true); // send the FIN
    while let Some(event) = parser.poll_event() {
        tester.visitor.handle_event(event);
    }
    assert_eq!(tester.visitor.messages_received, 2);
    let last_message = tester.visitor.last_message.as_ref().unwrap();
    assert!(message.equal_field_values(last_message));
    assert!(tester.visitor.object_payload.is_some());
    assert_eq!(tester.visitor.object_payload, Some(Bytes::from_static(b"")));
    assert!(tester.visitor.end_of_message);
    assert!(!tester.visitor.parsing_error.is_some());
    Ok(())
}
/*
// Send the header + some payload, pure payload, then pure payload to end the
// message.
TEST_F(MoqtMessageSpecificTest, ThreePartObject) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  auto message = std::make_unique<ObjectStreamMessage>();
  parser.ProcessData(message->PacketSample(), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(message.equal_field_values(*tester.visitor.last_message_));
  assert!(!tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "foo");

  // second part
  parser.ProcessData("bar", false);
  assert_eq!(tester.visitor.messages_received_, 2);
  assert!(message.equal_field_values(*tester.visitor.last_message_));
  assert!(!tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "bar");

  // third part includes FIN
  parser.ProcessData("deadbeef", true);
  assert_eq!(tester.visitor.messages_received_, 3);
  assert!(message.equal_field_values(*tester.visitor.last_message_));
  assert!(tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "deadbeef");
  assert!(!tester.visitor.parsing_error_.has_value());
}

// Send the part of header, rest of header + payload, plus payload.
TEST_F(MoqtMessageSpecificTest, ThreePartObjectFirstIncomplete) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  auto message = std::make_unique<ObjectStreamMessage>();

  // first part
  parser.ProcessData(message->PacketSample().substr(0, 4), false);
  assert_eq!(tester.visitor.messages_received_, 0);

  // second part. Add padding to it.
  message->set_wire_image_size(100);
  parser.ProcessData(
      message->PacketSample().substr(4, message->total_message_size() - 4),
      false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(message.equal_field_values(*tester.visitor.last_message_));
  assert!(!tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  // The value "93" is the overall wire image size of 100 minus the non-payload
  // part of the message.
  assert_eq!(tester.visitor.object_payload_->length(), 93);

  // third part includes FIN
  parser.ProcessData("bar", true);
  assert_eq!(tester.visitor.messages_received_, 2);
  assert!(message.equal_field_values(*tester.visitor.last_message_));
  assert!(tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "bar");
  assert!(!tester.visitor.parsing_error_.has_value());
}

TEST_F(MoqtMessageSpecificTest, StreamHeaderGroupFollowOn) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  // first part
  auto message1 = std::make_unique<StreamHeaderGroupMessage>();
  parser.ProcessData(message1->PacketSample(), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(message1.equal_field_values(*tester.visitor.last_message_));
  assert!(tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "foo");
  assert!(!tester.visitor.parsing_error_.has_value());
  // second part
  auto message2 = std::make_unique<StreamMiddlerGroupMessage>();
  parser.ProcessData(message2->PacketSample(), false);
  assert_eq!(tester.visitor.messages_received_, 2);
  assert!(message2.equal_field_values(*tester.visitor.last_message_));
  assert!(tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "bar");
  assert!(!tester.visitor.parsing_error_.has_value());
}

TEST_F(MoqtMessageSpecificTest, StreamHeaderTrackFollowOn) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  // first part
  auto message1 = std::make_unique<StreamHeaderTrackMessage>();
  parser.ProcessData(message1->PacketSample(), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(message1.equal_field_values(*tester.visitor.last_message_));
  assert!(tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "foo");
  assert!(!tester.visitor.parsing_error_.has_value());
  // second part
  auto message2 = std::make_unique<StreamMiddlerTrackMessage>();
  parser.ProcessData(message2->PacketSample(), false);
  assert_eq!(tester.visitor.messages_received_, 2);
  assert!(message2.equal_field_values(*tester.visitor.last_message_));
  assert!(tester.visitor.end_of_message_);
  assert!(tester.visitor.object_payload_.has_value());
  assert_eq!(*(tester.visitor.object_payload_), "bar");
  assert!(!tester.visitor.parsing_error_.has_value());
}

TEST_F(MoqtMessageSpecificTest, ClientSetupRoleIsInvalid) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x40, 0x02, 0x01, 0x02,  // versions
      0x03,                          // 3 params
      0x00, 0x01, 0x04,              // role = invalid
      0x01, 0x03, 0x66, 0x6f, 0x6f   // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "Invalid ROLE parameter");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, ServerSetupRoleIsInvalid) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x41, 0x01,
      0x01,                         // 1 param
      0x00, 0x01, 0x04,             // role = invalid
      0x01, 0x03, 0x66, 0x6f, 0x6f  // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "Invalid ROLE parameter");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, SetupRoleAppearsTwice) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x40, 0x02, 0x01, 0x02,  // versions
      0x03,                          // 3 params
      0x00, 0x01, 0x03,              // role = PubSub
      0x00, 0x01, 0x03,              // role = PubSub
      0x01, 0x03, 0x66, 0x6f, 0x6f   // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "ROLE parameter appears twice in SETUP");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, ClientSetupRoleIsMissing) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x40, 0x02, 0x01, 0x02,  // versions = 1, 2
      0x01,                          // 1 param
      0x01, 0x03, 0x66, 0x6f, 0x6f,  // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "ROLE parameter missing from CLIENT_SETUP message");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, ServerSetupRoleIsMissing) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x41, 0x01, 0x00,  // 1 param
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "ROLE parameter missing from SERVER_SETUP message");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, SetupRoleVarintLengthIsWrong) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x40,                   // type
      0x02, 0x01, 0x02,             // versions
      0x02,                         // 2 parameters
      0x00, 0x02, 0x03,             // role = PubSub, but length is 2
      0x01, 0x03, 0x66, 0x6f, 0x6f  // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "Parameter length does not match varint encoding");

  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kParameterLengthMismatch);
}

TEST_F(MoqtMessageSpecificTest, SetupPathFromServer) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x41,
      0x01,                          // version = 1
      0x01,                          // 1 param
      0x01, 0x03, 0x66, 0x6f, 0x6f,  // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "PATH parameter in SERVER_SETUP");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, SetupPathAppearsTwice) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x40, 0x02, 0x01, 0x02,  // versions = 1, 2
      0x03,                          // 3 params
      0x00, 0x01, 0x03,              // role = PubSub
      0x01, 0x03, 0x66, 0x6f, 0x6f,  // path = "foo"
      0x01, 0x03, 0x66, 0x6f, 0x6f,  // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "PATH parameter appears twice in CLIENT_SETUP");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, SetupPathOverWebtrans) {
  MoqtParser parser(K_WEB_TRANS, tester.visitor);
  char setup[] = {
      0x40, 0x40, 0x02, 0x01, 0x02,  // versions = 1, 2
      0x02,                          // 2 params
      0x00, 0x01, 0x03,              // role = PubSub
      0x01, 0x03, 0x66, 0x6f, 0x6f,  // path = "foo"
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "WebTransport connection is using PATH parameter in SETUP");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, SetupPathMissing) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char setup[] = {
      0x40, 0x40, 0x02, 0x01, 0x02,  // versions = 1, 2
      0x01,                          // 1 param
      0x00, 0x01, 0x03,              // role = PubSub
  };
  parser.ProcessData(absl::string_view(setup, sizeof(setup)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "PATH SETUP parameter missing from Client message over QUIC");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, SubscribeAuthorizationInfoTwice) {
  MoqtParser parser(K_WEB_TRANS, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02, 0x03, 0x66, 0x6f, 0x6f,  // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,              // track_name = "abcd"
      0x02,                                      // filter_type = kLatestObject
      0x02,                                      // two params
      0x02, 0x03, 0x62, 0x61, 0x72,              // authorization_info = "bar"
      0x02, 0x03, 0x62, 0x61, 0x72,              // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "AUTHORIZATION_INFO parameter appears twice in SUBSCRIBE");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, SubscribeUpdateAuthorizationInfoTwice) {
  MoqtParser parser(K_WEB_TRANS, tester.visitor);
  char subscribe_update[] = {
      0x02, 0x02, 0x03, 0x01, 0x05, 0x06,  // start and end sequences
      0x02,                                // 2 parameters
      0x02, 0x03, 0x62, 0x61, 0x72,        // authorization_info = "bar"
      0x02, 0x03, 0x62, 0x61, 0x72,        // authorization_info = "bar"
  };
  parser.ProcessData(
      absl::string_view(subscribe_update, sizeof(subscribe_update)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "AUTHORIZATION_INFO parameter appears twice in SUBSCRIBE_UPDATE");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, AnnounceAuthorizationInfoTwice) {
  MoqtParser parser(K_WEB_TRANS, tester.visitor);
  char announce[] = {
      0x06, 0x03, 0x66, 0x6f, 0x6f,  // track_namespace = "foo"
      0x02,                          // 2 params
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(announce, sizeof(announce)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "AUTHORIZATION_INFO parameter appears twice in ANNOUNCE");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, FinMidPayload) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  auto message = std::make_unique<StreamHeaderGroupMessage>();
  parser.ProcessData(
      message->PacketSample().substr(0, message->total_message_size() - 1),
      true);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "Received FIN mid-payload");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, PartialPayloadThenFin) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  auto message = std::make_unique<StreamHeaderTrackMessage>();
  parser.ProcessData(
      message->PacketSample().substr(0, message->total_message_size() - 1),
      false);
  parser.ProcessData(absl::string_view(), true);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "End of stream before complete OBJECT PAYLOAD");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, DataAfterFin) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  parser.ProcessData(absl::string_view(), true);  // Find FIN
  parser.ProcessData("foo", false);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "Data after end of stream");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, NonNormalObjectHasPayload) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char object_stream[] = {
      0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x02,  // varints
      0x66, 0x6f, 0x6f,                          // payload = "foo"
  };
  parser.ProcessData(absl::string_view(object_stream, sizeof(object_stream)),
                     false);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "Object with non-normal status has payload");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, InvalidObjectStatus) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char object_stream[] = {
      0x00, 0x03, 0x04, 0x05, 0x06, 0x07, 0x06,  // varints
      0x66, 0x6f, 0x6f,                          // payload = "foo"
  };
  parser.ProcessData(absl::string_view(object_stream, sizeof(object_stream)),
                     false);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "Invalid object status");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kProtocolViolation);
}

TEST_F(MoqtMessageSpecificTest, Setup2KB) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char big_message[2 * kMaxMessageHeaderSize];
  quic::QuicDataWriter writer(sizeof(big_message), big_message);
  writer.WriteVarInt62(static_cast<uint64_t>(MoqtMessageType::kServerSetup));
  writer.WriteVarInt62(0x1);                    // version
  writer.WriteVarInt62(0x1);                    // num_params
  writer.WriteVarInt62(0xbeef);                 // unknown param
  writer.WriteVarInt62(kMaxMessageHeaderSize);  // very long parameter
  writer.WriteRepeatedByte(0x04, kMaxMessageHeaderSize);
  // Send incomplete message
  parser.ProcessData(absl::string_view(big_message, writer.length() - 1),
                     false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "Cannot parse non-OBJECT messages > 2KB");
  assert_eq!(tester.visitor.parsing_error_code_, MoqtError::kInternalError);
}

TEST_F(MoqtMessageSpecificTest, UnknownMessageType) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char message[4];
  quic::QuicDataWriter writer(sizeof(message), message);
  writer.WriteVarInt62(0xbeef);  // unknown message type
  parser.ProcessData(absl::string_view(message, writer.length()), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "Unknown message type");
}

TEST_F(MoqtMessageSpecificTest, LatestGroup) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x01,                          // filter_type = kLatestGroup
      0x01,                          // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(tester.visitor.last_message_.has_value());
  MoqtSubscribe message =
      std::get<MoqtSubscribe>(tester.visitor.last_message_.value());
  assert!(!message.start_group.has_value());
  assert_eq!(message.start_object, 0);
  assert!(!message.end_group.has_value());
  assert!(!message.end_object.has_value());
}

TEST_F(MoqtMessageSpecificTest, LatestObject) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x02,                          // filter_type = kLatestObject
      0x01,                          // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(!tester.visitor.parsing_error_.has_value());
  MoqtSubscribe message =
      std::get<MoqtSubscribe>(tester.visitor.last_message_.value());
  assert!(!message.start_group.has_value());
  assert!(!message.start_object.has_value());
  assert!(!message.end_group.has_value());
  assert!(!message.end_object.has_value());
}

TEST_F(MoqtMessageSpecificTest, AbsoluteStart) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x03,                          // filter_type = kAbsoluteStart
      0x04,                          // start_group = 4
      0x01,                          // start_object = 1
      0x01,                          // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(!tester.visitor.parsing_error_.has_value());
  MoqtSubscribe message =
      std::get<MoqtSubscribe>(tester.visitor.last_message_.value());
  assert_eq!(message.start_group.value(), 4);
  assert_eq!(message.start_object.value(), 1);
  assert!(!message.end_group.has_value());
  assert!(!message.end_object.has_value());
}

TEST_F(MoqtMessageSpecificTest, AbsoluteRangeExplicitEndObject) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x04,                          // filter_type = kAbsoluteStart
      0x04,                          // start_group = 4
      0x01,                          // start_object = 1
      0x07,                          // end_group = 7
      0x03,                          // end_object = 2
      0x01,                          // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(!tester.visitor.parsing_error_.has_value());
  MoqtSubscribe message =
      std::get<MoqtSubscribe>(tester.visitor.last_message_.value());
  assert_eq!(message.start_group.value(), 4);
  assert_eq!(message.start_object.value(), 1);
  assert_eq!(message.end_group.value(), 7);
  assert_eq!(message.end_object.value(), 2);
}

TEST_F(MoqtMessageSpecificTest, AbsoluteRangeWholeEndGroup) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x04,                          // filter_type = kAbsoluteRange
      0x04,                          // start_group = 4
      0x01,                          // start_object = 1
      0x07,                          // end_group = 7
      0x00,                          // end whole group
      0x01,                          // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 1);
  assert!(!tester.visitor.parsing_error_.has_value());
  MoqtSubscribe message =
      std::get<MoqtSubscribe>(tester.visitor.last_message_.value());
  assert_eq!(message.start_group.value(), 4);
  assert_eq!(message.start_object.value(), 1);
  assert_eq!(message.end_group.value(), 7);
  assert!(!message.end_object.has_value());
}

TEST_F(MoqtMessageSpecificTest, AbsoluteRangeEndGroupTooLow) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x04,                          // filter_type = kAbsoluteRange
      0x04,                          // start_group = 4
      0x01,                          // start_object = 1
      0x03,                          // end_group = 3
      0x00,                          // end whole group
      0x01,                          // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "End group is less than start group");
}

TEST_F(MoqtMessageSpecificTest, AbsoluteRangeExactlyOneObject) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x04,                          // filter_type = kAbsoluteRange
      0x04,                          // start_group = 4
      0x01,                          // start_object = 1
      0x04,                          // end_group = 4
      0x02,                          // end object = 1
      0x00,                          // no parameters
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 1);
}

TEST_F(MoqtMessageSpecificTest, SubscribeUpdateExactlyOneObject) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe_update[] = {
      0x02, 0x02, 0x03, 0x01, 0x04, 0x07,  // start and end sequences
      0x00,                                // No parameters
  };
  parser.ProcessData(
      absl::string_view(subscribe_update, sizeof(subscribe_update)), false);
  assert_eq!(tester.visitor.messages_received_, 1);
}

TEST_F(MoqtMessageSpecificTest, SubscribeUpdateEndGroupTooLow) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe_update[] = {
      0x02, 0x02, 0x03, 0x01, 0x03, 0x06,  // start and end sequences
      0x01,                                // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,        // authorization_info = "bar"
  };
  parser.ProcessData(
      absl::string_view(subscribe_update, sizeof(subscribe_update)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "End group is less than start group");
}

TEST_F(MoqtMessageSpecificTest, AbsoluteRangeEndObjectTooLow) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe[] = {
      0x03, 0x01, 0x02,              // id and alias
      0x03, 0x66, 0x6f, 0x6f,        // track_namespace = "foo"
      0x04, 0x61, 0x62, 0x63, 0x64,  // track_name = "abcd"
      0x04,                          // filter_type = kAbsoluteRange
      0x04,                          // start_group = 4
      0x01,                          // start_object = 1
      0x04,                          // end_group = 4
      0x01,                          // end_object = 0
      0x01,                          // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,  // authorization_info = "bar"
  };
  parser.ProcessData(absl::string_view(subscribe, sizeof(subscribe)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "End object comes before start object");
}

TEST_F(MoqtMessageSpecificTest, SubscribeUpdateEndObjectTooLow) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe_update[] = {
      0x02, 0x02, 0x03, 0x02, 0x04, 0x01,  // start and end sequences
      0x01,                                // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,        // authorization_info = "bar"
  };
  parser.ProcessData(
      absl::string_view(subscribe_update, sizeof(subscribe_update)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_, "End object comes before start object");
}

TEST_F(MoqtMessageSpecificTest, SubscribeUpdateNoEndGroup) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char subscribe_update[] = {
      0x02, 0x02, 0x03, 0x02, 0x00, 0x01,  // start and end sequences
      0x01,                                // 1 parameter
      0x02, 0x03, 0x62, 0x61, 0x72,        // authorization_info = "bar"
  };
  parser.ProcessData(
      absl::string_view(subscribe_update, sizeof(subscribe_update)), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "SUBSCRIBE_UPDATE has end_object but no end_group");
}

TEST_F(MoqtMessageSpecificTest, AllMessagesTogether) {
  char buffer[5000];
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  size_t write = 0;
  size_t read = 0;
  int fully_received = 0;
  std::unique_ptr<TestMessageBase> prev_message = nullptr;
  for (MoqtMessageType type : message_types) {
    // Each iteration, process from the halfway point of one message to the
    // halfway point of the next.
    if (IsObjectMessage(type)) {
      continue;  // Objects cannot share a stream with other messages.
    }
    std::unique_ptr<TestMessageBase> message =
        CreateTestMessage(type, K_RAW_QUIC);
    memcpy(buffer + write, message->PacketSample().data(),
           message->total_message_size());
    size_t new_read = write + message->total_message_size() / 2;
    parser.ProcessData(absl::string_view(buffer + read, new_read - read),
                       false);
    assert_eq!(tester.visitor.messages_received_, fully_received);
    if (prev_message != nullptr) {
      assert!(prev_message.equal_field_values(*tester.visitor.last_message_));
    }
    fully_received++;
    read = new_read;
    write += message->total_message_size();
    prev_message = std::move(message);
  }
  // Deliver the rest
  parser.ProcessData(absl::string_view(buffer + read, write - read), true);
  assert_eq!(tester.visitor.messages_received_, fully_received);
  assert!(prev_message.equal_field_values(*tester.visitor.last_message_));
  assert!(!tester.visitor.parsing_error_.has_value());
}

TEST_F(MoqtMessageSpecificTest, DatagramSuccessful) {
  ObjectDatagramMessage message;
  MoqtObject object;
  absl::string_view payload =
      MoqtParser::ProcessDatagram(message.PacketSample(), object);
  TestMessageBase::MessageStructuredData object_metadata =
      TestMessageBase::MessageStructuredData(object);
  assert!(message.EqualFieldValues(object_metadata));
  assert_eq!(payload, "foo");
}

TEST_F(MoqtMessageSpecificTest, WrongMessageInDatagram) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  ObjectStreamMessage message;
  MoqtObject object;
  absl::string_view payload =
      MoqtParser::ProcessDatagram(message.PacketSample(), object);
  assert!(payload.empty());
}

TEST_F(MoqtMessageSpecificTest, TruncatedDatagram) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  ObjectDatagramMessage message;
  message.set_wire_image_size(4);
  MoqtObject object;
  absl::string_view payload =
      MoqtParser::ProcessDatagram(message.PacketSample(), object);
  assert!(payload.empty());
}

TEST_F(MoqtMessageSpecificTest, VeryTruncatedDatagram) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  char message = 0x40;
  MoqtObject object;
  absl::string_view payload = MoqtParser::ProcessDatagram(
      absl::string_view(&message, sizeof(message)), object);
  assert!(payload.empty());
}

TEST_F(MoqtMessageSpecificTest, SubscribeOkInvalidContentExists) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  SubscribeOkMessage subscribe_ok;
  subscribe_ok.SetInvalidContentExists();
  parser.ProcessData(subscribe_ok.PacketSample(), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "SUBSCRIBE_OK ContentExists has invalid value");
}

TEST_F(MoqtMessageSpecificTest, SubscribeDoneInvalidContentExists) {
  MoqtParser parser(K_RAW_QUIC, tester.visitor);
  SubscribeDoneMessage subscribe_done;
  subscribe_done.SetInvalidContentExists();
  parser.ProcessData(subscribe_done.PacketSample(), false);
  assert_eq!(tester.visitor.messages_received_, 0);
  assert!(tester.visitor.parsing_error_.has_value());
  assert_eq!(*tester.visitor.parsing_error_,
            "SUBSCRIBE_DONE ContentExists has invalid value");
}
 */
