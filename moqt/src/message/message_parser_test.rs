use crate::message::message_parser::{MessageParser, MessageParserEvent, ParserErrorCode};
use crate::message::message_test::{create_test_message, MessageStructuredData, TestMessageBase};
use crate::message::object::ObjectHeader;
use crate::message::{ControlMessage, MessageType};
use crate::Result;
use bytes::Bytes;
use std::fmt::{Display, Formatter};

static TEST_MESSAGE_TYPES: &[MessageType] = &[
    MessageType::ObjectStream, // ObjectDatagram is a unique set of tests.
    MessageType::StreamHeaderTrack,
    MessageType::StreamHeaderGroup,
    MessageType::Subscribe,
    MessageType::SubscribeOk,
    /*TODO:MessageType::SubscribeError,
    MessageType::SubscribeUpdate,
    MessageType::UnSubscribe,
    MessageType::SubscribeDone,
    MessageType::AnnounceCancel,
    MessageType::TrackStatusRequest,
    MessageType::TrackStatus,
    MessageType::Announce,
    MessageType::AnnounceOk,
    MessageType::AnnounceError,
    MessageType::UnAnnounce,*/
    MessageType::ClientSetup,
    MessageType::ServerSetup,
    /*MessageType::GoAway,*/
];

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

fn get_test_parser_params() -> Vec<TestParserParams> {
    let mut params = vec![];

    let uses_web_transport_bool = vec![false, true];
    for &message_type in TEST_MESSAGE_TYPES {
        if message_type == MessageType::ClientSetup {
            for uses_web_transport in &uses_web_transport_bool {
                params.push(TestParserParams::new(message_type, *uses_web_transport));
            }
        } else {
            // All other types are processed the same for either perspective or
            // transport.
            params.push(TestParserParams::new(message_type, true));
        }
    }
    params
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

#[test]
fn test_parse_one_message() -> Result<()> {
    for params in get_test_parser_params() {
        let mut tester = TestParser::new(&params);

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
    }
    Ok(())
}
