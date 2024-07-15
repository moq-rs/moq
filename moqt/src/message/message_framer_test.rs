use crate::message::message_framer::MessageFramer;
use crate::message::message_test::{
    create_test_message, MessageStructuredData, TestMessageBase, TestStreamHeaderGroupMessage,
    TestStreamMiddlerGroupMessage,
};
use crate::message::MessageType;
use crate::{Error, Result};
use bytes::{BufMut, Bytes};
use rstest::rstest;

struct TestFramerParams {
    message_type: MessageType,
    uses_web_transport: bool,
}

impl TestFramerParams {
    fn new(message_type: MessageType, uses_web_transport: bool) -> Self {
        Self {
            message_type,
            uses_web_transport,
        }
    }
}

struct TestFramer {
    message_type: MessageType,
    uses_web_transport: bool,
}

impl TestFramer {
    fn new(params: &TestFramerParams) -> Self {
        Self {
            message_type: params.message_type,
            uses_web_transport: params.uses_web_transport,
        }
    }

    fn make_message(&self) -> Box<dyn TestMessageBase> {
        create_test_message(self.message_type, self.uses_web_transport)
    }

    fn serialize_message<W: BufMut>(
        &self,
        structured_data: &MessageStructuredData,
        w: &mut W,
    ) -> Result<usize> {
        match self.message_type {
            MessageType::ObjectStream
            | MessageType::StreamHeaderTrack
            | MessageType::StreamHeaderGroup => {
                let object_header =
                    if let MessageStructuredData::Object(object_header) = structured_data {
                        object_header
                    } else {
                        return Err(Error::ErrInvalidMessageType(self.message_type as u64));
                    };
                MessageFramer::serialize_object(object_header, true, Bytes::from_static(b"foo"), w)
            }
            MessageType::ObjectDatagram => {
                Err(Error::ErrInvalidMessageType(self.message_type as u64))
            }
            _ => {
                let control_message =
                    if let MessageStructuredData::Control(control_message) = structured_data {
                        control_message
                    } else {
                        return Err(Error::ErrInvalidMessageType(self.message_type as u64));
                    };
                MessageFramer::serialize_control_message(control_message, w)
            }
        }
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
fn test_framer_one_message(params: (MessageType, bool)) -> Result<()> {
    let tester = TestFramer::new(&TestFramerParams::new(params.0, params.1));
    let message = tester.make_message();
    let structured_data = message.structured_data();
    let mut buffer = vec![];
    let size = tester.serialize_message(&structured_data, &mut buffer)?;
    assert_eq!(size, buffer.len());
    assert_eq!(buffer.len(), message.packet_sample().len());
    assert_eq!(&buffer[..], message.packet_sample());
    Ok(())
}

#[test]
fn test_group_middler() -> Result<()> {
    let header = TestStreamHeaderGroupMessage::new();
    let header_object_header =
        if let MessageStructuredData::Object(object_header) = header.structured_data() {
            object_header
        } else {
            assert!(false);
            return Err(Error::ErrInvalidMessageType(0));
        };

    let mut buffer1 = vec![];
    let buffer1_size = MessageFramer::serialize_object(
        &header_object_header,
        true,
        Bytes::from_static(b"foo"),
        &mut buffer1,
    )?;
    assert_eq!(buffer1_size, buffer1.len());
    assert_eq!(buffer1.len(), header.total_message_size());
    assert_eq!(&buffer1[..], header.packet_sample());

    let middler = TestStreamMiddlerGroupMessage::new();
    let middler_object_header =
        if let MessageStructuredData::Object(object_header) = middler.structured_data() {
            object_header
        } else {
            assert!(false);
            return Err(Error::ErrInvalidMessageType(0));
        };
    let mut buffer2 = vec![];
    let buffer2_size = MessageFramer::serialize_object(
        &middler_object_header,
        false,
        Bytes::from_static(b"bar"),
        &mut buffer2,
    )?;
    assert_eq!(buffer2_size, buffer2.len());
    assert_eq!(buffer2.len(), middler.total_message_size());
    assert_eq!(&buffer2[..], middler.packet_sample());
    Ok(())
}
/*
#[test]
fn test_TrackMiddler() -> Result<()> {
  auto header = std::make_unique<StreamHeaderTrackMessage>();
  auto buffer1 = SerializeObject(
      framer_, std::get<MoqtObject>(header->structured_data()), "foo", true);
  assert_eq!(buffer1.size(), header->total_message_size());
  assert_eq!(buffer1.AsStringView(), header.packet_sample());

  auto middler = std::make_unique<StreamMiddlerTrackMessage>();
  auto buffer2 = SerializeObject(
      framer_, std::get<MoqtObject>(middler->structured_data()), "bar", false);
  assert_eq!(buffer2.size(), middler->total_message_size());
  assert_eq!(buffer2.AsStringView(), middler.packet_sample());
    Ok(())
}

#[test]
fn test_BadObjectInput() -> Result<()> {
  MoqtObject object = {
      /*subscribe_id=*/3,
      /*track_alias=*/4,
      /*group_id=*/5,
      /*object_id=*/6,
      /*object_send_order=*/7,
      /*object_status=*/MoqtObjectStatus::kNormal,
      /*forwarding_preference=*/MoqtForwardingPreference::kObject,
      /*payload_length=*/std::nullopt,
  };
  quiche::QuicheBuffer buffer;
  object.forwarding_preference = MoqtForwardingPreference::kDatagram;
  EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectHeader(object, false),
                  "must be first");
  assert!(buffer.empty());
  object.forwarding_preference = MoqtForwardingPreference::kGroup;
  EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectHeader(object, false),
                  "requires knowing the object length");
  assert!(buffer.empty());
  object.payload_length = 5;
  object.object_status = MoqtObjectStatus::kEndOfGroup;
  EXPECT_QUIC_BUG(buffer = framer_.SerializeObjectHeader(object, false),
                  "Object status must be kNormal if payload is non-empty");
  assert!(buffer.empty());
    Ok(())
}

#[test]
fn test_Datagram() -> Result<()> {
  auto datagram = std::make_unique<ObjectDatagramMessage>();
  MoqtObject object = {
      /*subscribe_id=*/3,
      /*track_alias=*/4,
      /*group_id=*/5,
      /*object_id=*/6,
      /*object_send_order=*/7,
      /*object_status=*/MoqtObjectStatus::kNormal,
      /*forwarding_preference=*/MoqtForwardingPreference::kObject,
      /*payload_length=*/std::nullopt,
  };
  std::string payload = "foo";
  quiche::QuicheBuffer buffer;
  buffer = framer_.SerializeObjectDatagram(object, payload);
  assert_eq!(buffer.size(), datagram->total_message_size());
  assert_eq!(buffer.AsStringView(), datagram.packet_sample());
    Ok(())
}

#[test]
fn test_AllSubscribeInputs() -> Result<()> {
  for (std::optional<uint64_t> start_group :
       {std::optional<uint64_t>(), std::optional<uint64_t>(4)}) {
    for (std::optional<uint64_t> start_object :
         {std::optional<uint64_t>(), std::optional<uint64_t>(0)}) {
      for (std::optional<uint64_t> end_group :
           {std::optional<uint64_t>(), std::optional<uint64_t>(7)}) {
        for (std::optional<uint64_t> end_object :
             {std::optional<uint64_t>(), std::optional<uint64_t>(3)}) {
          MoqtSubscribe subscribe = {
              /*subscribe_id=*/3,
              /*track_alias=*/4,
              /*track_namespace=*/"foo",
              /*track_name=*/"abcd",
              start_group,
              start_object,
              end_group,
              end_object,
              /*authorization_info=*/"bar",
          };
          quiche::QuicheBuffer buffer;
          MoqtFilterType expected_filter_type = MoqtFilterType::kNone;
          if (!start_group.has_value() && !start_object.has_value() &&
              !end_group.has_value() && !end_object.has_value()) {
            expected_filter_type = MoqtFilterType::kLatestObject;
          } else if (!start_group.has_value() && start_object.has_value() &&
                     *start_object == 0 && !end_group.has_value() &&
                     !end_object.has_value()) {
            expected_filter_type = MoqtFilterType::kLatestGroup;
          } else if (start_group.has_value() && start_object.has_value() &&
                     !end_group.has_value() && !end_object.has_value()) {
            expected_filter_type = MoqtFilterType::kAbsoluteStart;
          } else if (start_group.has_value() && start_object.has_value() &&
                     end_group.has_value()) {
            expected_filter_type = MoqtFilterType::kAbsoluteRange;
          }
          if (expected_filter_type == MoqtFilterType::kNone) {
            EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                            "Invalid object range");
            assert_eq!(buffer.size(), 0);
            continue;
          }
          buffer = framer_.SerializeSubscribe(subscribe);
          // Go to the filter type.
          const uint8_t* read = BufferAtOffset(buffer, 12);
          assert_eq!(static_cast<MoqtFilterType>(*read), expected_filter_type);
          EXPECT_GT(buffer.size(), 0);
          if (expected_filter_type == MoqtFilterType::kAbsoluteRange &&
              end_object.has_value()) {
            const uint8_t* object_id = read + 4;
            assert_eq!(*object_id, *end_object + 1);
          }
        }
      }
    }
  }
    Ok(())
}

#[test]
fn test_SubscribeEndBeforeStart() -> Result<()> {
  MoqtSubscribe subscribe = {
      /*subscribe_id=*/3,
      /*track_alias=*/4,
      /*track_namespace=*/"foo",
      /*track_name=*/"abcd",
      /*start_group=*/std::optional<uint64_t>(4),
      /*start_object=*/std::optional<uint64_t>(3),
      /*end_group=*/std::optional<uint64_t>(3),
      /*end_object=*/std::nullopt,
      /*authorization_info=*/"bar",
  };
  quiche::QuicheBuffer buffer;
  EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                  "Invalid object range");
  assert_eq!(buffer.size(), 0);
  subscribe.end_group = 4;
  subscribe.end_object = 1;
  EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                  "Invalid object range");
  assert_eq!(buffer.size(), 0);
    Ok(())
}

#[test]
fn test_SubscribeLatestGroupNonzeroObject() -> Result<()> {
  MoqtSubscribe subscribe = {
      /*subscribe_id=*/3,
      /*track_alias=*/4,
      /*track_namespace=*/"foo",
      /*track_name=*/"abcd",
      /*start_group=*/std::nullopt,
      /*start_object=*/std::optional<uint64_t>(3),
      /*end_group=*/std::nullopt,
      /*end_object=*/std::nullopt,
      /*authorization_info=*/"bar",
  };
  quiche::QuicheBuffer buffer;
  EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribe(subscribe),
                  "Invalid object range");
  assert_eq!(buffer.size(), 0);
    Ok(())
}

#[test]
fn test_SubscribeUpdateEndGroupOnly() -> Result<()> {
  MoqtSubscribeUpdate subscribe_update = {
      /*subscribe_id=*/3,
      /*start_group=*/4,
      /*start_object=*/3,
      /*end_group=*/4,
      /*end_object=*/std::nullopt,
      /*authorization_info=*/"bar",
  };
  quiche::QuicheBuffer buffer;
  buffer = framer_.SerializeSubscribeUpdate(subscribe_update);
  EXPECT_GT(buffer.size(), 0);
  const uint8_t* end_group = BufferAtOffset(buffer, 4);
  assert_eq!(*end_group, 5);
  const uint8_t* end_object = end_group + 1;
  assert_eq!(*end_object, 0);
    Ok(())
}

#[test]
fn test_SubscribeUpdateIncrementsEnd() -> Result<()> {
  MoqtSubscribeUpdate subscribe_update = {
      /*subscribe_id=*/3,
      /*start_group=*/4,
      /*start_object=*/3,
      /*end_group=*/4,
      /*end_object=*/6,
      /*authorization_info=*/"bar",
  };
  quiche::QuicheBuffer buffer;
  buffer = framer_.SerializeSubscribeUpdate(subscribe_update);
  EXPECT_GT(buffer.size(), 0);
  const uint8_t* end_group = BufferAtOffset(buffer, 4);
  assert_eq!(*end_group, 5);
  const uint8_t* end_object = end_group + 1;
  assert_eq!(*end_object, 7);
    Ok(())
}

#[test]
fn test_SubscribeUpdateInvalidRange() -> Result<()> {
  MoqtSubscribeUpdate subscribe_update = {
      /*subscribe_id=*/3,
      /*start_group=*/4,
      /*start_object=*/3,
      /*end_group=*/std::nullopt,
      /*end_object=*/6,
      /*authorization_info=*/"bar",
  };
  quiche::QuicheBuffer buffer;
  EXPECT_QUIC_BUG(buffer = framer_.SerializeSubscribeUpdate(subscribe_update),
                  "Invalid object range");
  assert_eq!(buffer.size(), 0);
    Ok(())
}
 */
