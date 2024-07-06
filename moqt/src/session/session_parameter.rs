use crate::message::Version;

pub enum Perspective {
    Server,
    Client,
}

pub struct SessionParameter {
    pub version: Version,
    pub perspective: Perspective,
    pub use_web_transport: bool,
    pub path: String,
    pub deliver_partial_objects: bool,
}
