use crate::connection::Connection;
use crate::driver::SessionDriver;
use crate::handler::Handler;
use crate::protocol::{self, Command, EventIn, EventOut, ReadInput, ReadOutput};
use crate::Result;
use retty::transport::Transmit;
use std::time::Instant;

pub mod config;
pub(crate) mod local_track;
pub(crate) mod remote_track;
mod subscribe_window;

impl From<config::Perspective> for protocol::Perspective {
    fn from(value: config::Perspective) -> Self {
        match value {
            config::Perspective::Server => Self::Server,
            config::Perspective::Client => Self::Client,
        }
    }
}

impl From<config::Config> for protocol::Config {
    fn from(value: config::Config) -> Self {
        Self {
            version: value.version,
            perspective: value.perspective.into(),
            use_web_transport: value.use_web_transport,
            path: value.path,
            deliver_partial_objects: value.deliver_partial_objects,
        }
    }
}

pub(crate) struct Session {
    driver: SessionDriver<Connection>,
}

impl Session {
    pub(crate) fn new(config: config::Config, conn: Connection) -> Self {
        Self {
            driver: SessionDriver::new(config.into(), conn),
        }
    }

    pub(crate) fn transport(&self) -> &Connection {
        self.driver.transport()
    }

    pub(crate) fn transport_mut(&mut self) -> &mut Connection {
        self.driver.transport_mut()
    }

    pub(crate) fn into_transport(self) -> Connection {
        self.driver.into_transport()
    }
}

impl Handler for Session {
    type Ein = EventIn;
    type Eout = EventOut;
    type Rin = ReadInput;
    type Rout = ReadOutput;
    type Win = Command;
    type Wout = ();

    fn transport_active(&mut self) -> Result<()> {
        self.driver.on_transport_connected()
    }

    fn transport_inactive(&mut self) -> Result<()> {
        self.driver.on_transport_closed()
    }

    fn handle_read(&mut self, msg: Transmit<Self::Rin>) -> Result<()> {
        match msg.message {
            ReadInput::StreamData {
                stream_id,
                data,
                fin,
            } => self.driver.on_stream_data(stream_id, data, fin),
            ReadInput::Datagram(bytes) => self.driver.on_datagram(bytes),
        }
    }

    fn poll_read(&mut self) -> Option<Transmit<Self::Rout>> {
        self.driver.poll_read().map(|message| Transmit {
            now: Instant::now(),
            transport: self.driver.transport().transport(),
            message,
        })
    }

    fn handle_write(&mut self, msg: Transmit<Self::Win>) -> Result<()> {
        self.driver.handle_command(msg.message)
    }

    fn poll_write(&mut self) -> Option<Transmit<Self::Wout>> {
        None
    }

    fn handle_event(&mut self, evt: Self::Ein) -> Result<()> {
        match evt {
            EventIn::TransportConnected => self.driver.on_transport_connected(),
            EventIn::TransportClosed => self.driver.on_transport_closed(),
            EventIn::StreamOpened {
                stream_id,
                bidi,
                local,
            } => self.driver.on_stream_opened(stream_id, bidi, local),
            EventIn::StreamClosed { stream_id } => self.driver.on_stream_closed(stream_id),
        }
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.driver.poll_event()
    }

    fn handle_timeout(&mut self, now: Instant) -> Result<()> {
        self.driver.handle_timeout(now)
    }

    fn poll_timeout(&mut self) -> Option<Instant> {
        self.driver.poll_timeout()
    }
}
