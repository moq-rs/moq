#![allow(non_upper_case_globals)]

pub const kDefaultMoqtVersion: &str = kDraft16;
pub const kImplementationName: &str = "moq.rs MOQT draft 16";

pub const kDraft16: &str = "moqt-16";
pub const kUnrecognizedVersionForTests: &str = "moqt-15";

pub const kDefaultInitialMaxRequestId: u64 = 100;
pub const kDefaultMaxRequestId: u64 = 0;

pub const kDefaultMaxAuthTokenCacheSize: u64 = 0;
pub const kDefaultSupportObjectAcks: bool = false;
pub const kDefaultInitialMaxSubscribeId: u64 = 100;

/// The maximum length of a message, excluding any OBJECT payload. This prevents
/// DoS attack via forcing the parser to buffer a large message (OBJECT payloads
/// are not buffered by the parser).
pub const kMaxMessageHeaderSize: usize = 2048;
