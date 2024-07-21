use crate::handler::Handler;
use crate::message::announce_error::AnnounceErrorReason;
use crate::message::client_setup::ClientSetup;
use crate::message::object::ObjectForwardingPreference;
use crate::message::subscribe::Subscribe;
use crate::message::{ControlMessage, FullTrackName, Role};
use crate::session::config::{Config, Perspective};
use crate::session::local_track::LocalTrack;
use crate::session::remote_track::RemoteTrack;
use crate::Result;
use crate::StreamId;
use log::info;
use retty::transport::Transmit;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

mod config;
mod connection;
mod local_track;
mod remote_track;
mod stream;
mod subscribe_window;

// If |error_message| is none, the ANNOUNCE was successful.
pub type OutgoingAnnounceCallback = fn(track_namespace: String, error: Option<AnnounceErrorReason>);

/// Indexed by subscribe_id.
pub struct ActiveSubscribe {
    message: Subscribe,
    // The forwarding preference of the first received object, which all
    // subsequent objects must match.
    forwarding_preference: Option<ObjectForwardingPreference>,
    // If true, an object has arrived for the subscription before SUBSCRIBE_OK
    // arrived.
    received_object: bool,
}

pub struct Session {
    config: Config,
    control_stream: Option<StreamId>,

    // All the tracks the session is subscribed to, indexed by track_alias.
    // Multiple subscribes to the same track are recorded in a single
    // subscription.
    remote_tracks: HashMap<u64, RemoteTrack>,
    // Look up aliases for remote tracks by name
    remote_track_aliases: HashMap<FullTrackName, u64>,
    next_remote_track_alias: u64,

    // All the tracks the peer can subscribe to.
    local_tracks: HashMap<FullTrackName, LocalTrack>,
    local_track_by_subscribe_id: HashMap<u64, FullTrackName>,
    // This is only used to check for track_alias collisions.
    used_track_aliases: HashSet<u64>,
    next_local_track_alias: u64,

    // Outgoing SUBSCRIBEs that have not received SUBSCRIBE_OK or SUBSCRIBE_ERROR.
    active_subscribes: HashMap<u64, ActiveSubscribe>,
    next_subscribe_id: u64,

    // Indexed by track namespace.
    pending_outgoing_announces: HashMap<String, OutgoingAnnounceCallback>,

    // The role the peer advertised in its SETUP message. Initialize it to avoid
    // an uninitialized value if no SETUP arrives or it arrives with no Role
    // parameter, and other checks have changed/been disabled.
    peer_role: Role,
}

impl Session {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            control_stream: None,
            remote_tracks: Default::default(),
            remote_track_aliases: Default::default(),
            next_remote_track_alias: 0,
            local_tracks: Default::default(),
            local_track_by_subscribe_id: Default::default(),
            used_track_aliases: Default::default(),
            next_local_track_alias: 0,
            active_subscribes: Default::default(),
            next_subscribe_id: 0,
            pending_outgoing_announces: Default::default(),
            peer_role: Default::default(),
        }
    }

    fn send_control_message(&self, _control_message: ControlMessage) {}
}

impl Handler for Session {
    type Ein = ();
    type Eout = ();
    type Rin = ();
    type Rout = ();
    type Win = ();
    type Wout = ();

    fn transport_active(&mut self) -> Result<()> {
        info!("{:?} Underlying session ready", self.config.perspective);
        if self.config.perspective == Perspective::Server {
            return Ok(());
        }

        let mut setup = ClientSetup {
            supported_versions: vec![self.config.version],
            role: Some(Role::PubSub),
            path: None,
            uses_web_transport: self.config.use_web_transport,
        };
        if !self.config.use_web_transport {
            setup.path = Some(self.config.path.clone());
        }
        self.send_control_message(ControlMessage::ClientSetup(setup));
        info!("{:?} Send the SETUP message", self.config.perspective);
        Ok(())
    }

    fn transport_inactive(&mut self) -> Result<()> {
        todo!()
    }

    fn handle_read(&mut self, _msg: Transmit<Self::Rin>) -> Result<()> {
        todo!()
    }

    fn poll_read(&mut self) -> Option<Transmit<Self::Rout>> {
        todo!()
    }

    fn handle_write(&mut self, _msg: Transmit<Self::Win>) -> Result<()> {
        todo!()
    }

    fn poll_write(&mut self) -> Option<Transmit<Self::Wout>> {
        todo!()
    }

    fn handle_event(&mut self, _evt: Self::Ein) -> Result<()> {
        todo!()
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        todo!()
    }

    fn handle_timeout(&mut self, _now: Instant) -> Result<()> {
        todo!()
    }

    fn poll_timeout(&mut self) -> Option<Instant> {
        todo!()
    }
}
