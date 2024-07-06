use crate::message::object::ObjectForwardingPreference;
use crate::message::FullTrackName;
use bytes::Bytes;

pub trait RemoteTrackVisitor {
    fn on_reply(&mut self, full_track_name: &FullTrackName, error_reason_phrase: Option<String>);

    #[allow(clippy::too_many_arguments)]
    fn on_object_fragment(
        &mut self,
        full_track_name: &FullTrackName,
        group_id: u64,
        object_id: u64,
        object_send_order: u64,
        object_status: u64,
        forwarding_preference: ObjectForwardingPreference,
        object_payload: Bytes,
        end_of_message: bool,
    );
}

/// A track on the peer to which the session has subscribed.
pub struct RemoteTrack {
    full_track_name: FullTrackName,
    track_alias: u64,
    forwarding_preference: Option<ObjectForwardingPreference>,
}

impl RemoteTrack {
    pub fn new(full_track_name: FullTrackName, track_alias: u64) -> Self {
        Self {
            full_track_name,
            track_alias,
            forwarding_preference: None,
        }
    }

    pub fn full_track_name(&self) -> &FullTrackName {
        &self.full_track_name
    }

    pub fn track_alias(&self) -> u64 {
        self.track_alias
    }

    /// When called while processing the first object in the track, sets the
    /// forwarding preference to the value indicated by the incoming encoding.
    /// Otherwise, returns true if the incoming object does not violate the rule
    /// that the preference is consistent.
    pub fn check_forwarding_preference(&mut self, preference: ObjectForwardingPreference) -> bool {
        if let Some(forwarding_preference) = self.forwarding_preference.as_ref() {
            return *forwarding_preference == preference;
        }
        self.forwarding_preference = Some(preference);
        true
    }
}
