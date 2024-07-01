mod datagram;
mod group;
mod stream;
mod track;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ObjectForwardingPreference {
    #[default]
    Object,
    Datagram,
    Track,
    Group,
}
