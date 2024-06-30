mod announce;
mod announce_cancel;
mod announce_error;
mod announce_ok;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum MessageType {
    ObjectStream = 0x0,
    ObjectDatagram = 0x1,
    Subscribe = 0x3,
    SubscribeOk = 0x4,
    SubscribeError = 0x5,
    Announce = 0x6,
    AnnounceOk = 0x7,
    AnnounceError = 0x8,
    UnAnnounce = 0x9,
    UnSubscribe = 0xa,
    SubscribeDone = 0xb,
    AnnounceCancel = 0xc,
    TrackStatusRequest = 0xd,
    TrackStatus = 0xe,
    GoAway = 0x10,
    ClientSetup = 0x40,
    ServerSetup = 0x41,
    StreamHeaderTrack = 0x50,
    StreamHeaderGroup = 0x51,
}
