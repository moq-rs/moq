use crate::message::announce_error::AnnounceErrorReason;
use crate::message::object::ObjectForwardingPreference;
use crate::message::subscribe::Subscribe;
use crate::message::{FullTrackName, Role};
use crate::session::local_track::LocalTrack;
use crate::session::remote_track::{RemoteTrack, RemoteTrackVisitor};
use crate::session::session_parameters::SessionParameters;
use crate::StreamId;
use std::collections::{HashMap, HashSet};

mod local_track;
mod remote_track;
mod session_parameters;
mod stream;
mod subscribe_window;

// If |error_message| is none, the ANNOUNCE was successful.
pub type OutgoingAnnounceCallback = fn(track_namespace: String, error: Option<AnnounceErrorReason>);

/// Indexed by subscribe_id.
pub struct ActiveSubscribe {
    message: Subscribe,
    visitor: Box<dyn RemoteTrackVisitor>,
    // The forwarding preference of the first received object, which all
    // subsequent objects must match.
    forwarding_preference: Option<ObjectForwardingPreference>,
    // If true, an object has arrived for the subscription before SUBSCRIBE_OK
    // arrived.
    received_object: bool,
}

pub struct Session {
    parameters: SessionParameters,
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
    pub fn new(parameters: SessionParameters) -> Self {
        Self {
            parameters,
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
}
