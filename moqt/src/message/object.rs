#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ObjectForwardingPreference {
    #[default]
    Object,
    Datagram,
    Track,
    Group,
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ObjectStatus {
    #[default]
    Normal = 0x0,
    ObjectDoesNotExist = 0x1,
    GroupDoesNotExist = 0x2,
    EndOfGroup = 0x3,
    EndOfTrack = 0x4,
    Invalid = 0x5,
}

impl From<u64> for ObjectStatus {
    fn from(value: u64) -> Self {
        match value {
            0x0 => Self::Normal,
            0x1 => Self::ObjectDoesNotExist,
            0x2 => Self::GroupDoesNotExist,
            0x3 => Self::EndOfGroup,
            0x4 => Self::EndOfTrack,
            _ => Self::Invalid,
        }
    }
}

/// The data contained in every Object message, although the message type
/// implies some of the values. |payload_length| has no value if the length
/// is unknown (because it runs to the end of the stream.)
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct ObjectHeader {
    pub subscribe_id: u64,
    pub track_alias: u64,
    pub group_id: u64,
    pub object_id: u64,
    pub object_send_order: u64,
    pub object_status: ObjectStatus,
    pub object_forwarding_preference: ObjectForwardingPreference,
    pub object_payload_length: Option<u64>,
}
