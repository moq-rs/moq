pub mod datagram;
pub mod group;
pub mod stream;
pub mod track;

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
