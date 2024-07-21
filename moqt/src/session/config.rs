use crate::message::Version;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum Perspective {
    #[default]
    Server,
    Client,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub version: Version,
    pub perspective: Perspective,
    pub use_web_transport: bool,
    pub path: String,
    pub deliver_partial_objects: bool,
}
