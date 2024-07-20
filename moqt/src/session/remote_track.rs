use crate::message::object::{ObjectForwardingPreference, ObjectHeader};
use crate::message::FullTrackName;
use bytes::Bytes;

pub struct RemoteTrackOnReply {
    pub full_track_name: FullTrackName,
    pub error_reason_phrase: Option<String>,
}

pub struct RemoteTrackOnObjectFragment {
    pub object_header: ObjectHeader,
    pub payload: Bytes,
    pub fin: bool,
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::Result;

    struct RemoteTrackTest {
        track: RemoteTrack,
    }

    impl RemoteTrackTest {
        fn new() -> Self {
            Self {
                track: RemoteTrack::new(
                    FullTrackName::new("foo".to_string(), "bar".to_string()),
                    5,
                ),
            }
        }
    }

    #[test]
    fn test_remote_track_test_queries() -> Result<()> {
        let track = &mut RemoteTrackTest::new().track;
        assert_eq!(
            track.full_track_name(),
            &FullTrackName::new("foo".to_string(), "bar".to_string())
        );
        assert_eq!(track.track_alias(), 5);
        Ok(())
    }

    #[test]
    fn test_remote_track_test_update_forwarding_preference() -> Result<()> {
        let track = &mut RemoteTrackTest::new().track;
        assert!(track.check_forwarding_preference(ObjectForwardingPreference::Object));
        assert!(track.check_forwarding_preference(ObjectForwardingPreference::Object));
        assert!(!track.check_forwarding_preference(ObjectForwardingPreference::Datagram));
        Ok(())
    }
}
