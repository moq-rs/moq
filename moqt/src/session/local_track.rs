use crate::message::object::{ObjectForwardingPreference, ObjectStatus};
use crate::message::{FullSequence, FullTrackName};
use crate::session::subscribe_window::{SubscribeWindow, SubscribeWindows};
use log::error;
use std::collections::HashMap;

/// A track to which the peer might subscribe.
pub struct LocalTrack {
    // This only needs to track subscriptions to current and future objects;
    // requests for objects in the past are forwarded to the application.
    full_track_name: FullTrackName,
    // The forwarding preference for the track.
    forwarding_preference: ObjectForwardingPreference,
    // Let the first SUBSCRIBE determine the track alias.
    track_alias: Option<u64>,
    // The sequence numbers from this track to which the peer is subscribed.
    windows: SubscribeWindows,
    // By recording the highest observed sequence number, MoQT can interpret
    // relative sequence numbers in SUBSCRIBEs.
    next_sequence: FullSequence,
    // The object ID of each EndOfGroup object received, indexed by group ID.
    // Entry does not exist, if no kGroupDoesNotExist, EndOfGroup, or
    // EndOfTrack has been received for that group.
    max_object_ids: HashMap<u64, u64>,

    // If true, the session has received ANNOUNCE_CANCELED for this namespace.
    // Additional subscribes will be a protocol error, and the track can be
    // destroyed once all active subscribes end.
    announce_canceled: bool,
}

impl LocalTrack {
    pub fn new(
        full_track_name: FullTrackName,
        forwarding_preference: ObjectForwardingPreference,
        next_sequence: Option<FullSequence>,
    ) -> Self {
        Self {
            full_track_name,
            forwarding_preference,
            track_alias: None,
            windows: SubscribeWindows::new(forwarding_preference),
            next_sequence: if let Some(next_sequence) = next_sequence {
                next_sequence
            } else {
                FullSequence {
                    group_id: 0,
                    object_id: 0,
                }
            },
            max_object_ids: Default::default(),
            announce_canceled: false,
        }
    }

    pub fn full_track_name(&self) -> &FullTrackName {
        &self.full_track_name
    }

    pub fn track_alias(&self) -> Option<u64> {
        self.track_alias
    }

    pub fn set_track_alias(&mut self, track_alias: u64) {
        self.track_alias = Some(track_alias);
    }

    /// Returns the subscribe windows that want the object defined by (|group|,
    /// |object|).
    pub fn should_send(&self, sequence: FullSequence) -> Vec<&SubscribeWindow> {
        self.windows.sequence_is_subscribed(sequence)
    }

    pub fn add_window(
        &mut self,
        subscribe_id: u64,
        start: FullSequence,
        end_group: Option<u64>,
        end_object: Option<u64>,
    ) {
        if self.announce_canceled {
            error!("Canceled track got subscription")
        }
        if let Some(end_group) = end_group {
            if let Some(end_object) = end_object {
                self.windows.add_window(
                    subscribe_id,
                    self.next_sequence,
                    start,
                    Some(FullSequence {
                        group_id: end_group,
                        object_id: end_object,
                    }),
                );
            } else {
                let max_object_id = self.max_object_ids.get(&end_group);
                if end_group >= self.next_sequence.group_id || max_object_id.is_none() {
                    self.windows.add_window(
                        subscribe_id,
                        self.next_sequence,
                        start,
                        Some(FullSequence {
                            group_id: end_group,
                            object_id: u64::MAX,
                        }),
                    );
                } else if let Some(max_object_id) = max_object_id {
                    self.windows.add_window(
                        subscribe_id,
                        self.next_sequence,
                        start,
                        Some(FullSequence {
                            group_id: end_group,
                            object_id: *max_object_id,
                        }),
                    );
                }
            }
        } else {
            self.windows
                .add_window(subscribe_id, self.next_sequence, start, None);
        }
    }

    pub fn delete_window(&mut self, subscribe_id: u64) {
        self.windows.remove_window(subscribe_id);
    }

    /// Returns the largest observed sequence, but increments the object sequence
    /// by one.
    pub fn next_sequence(&self) -> &FullSequence {
        &self.next_sequence
    }

    /// Updates next_sequence_ if |sequence| is larger. Updates max_object_ids_
    /// if relevant.
    pub fn sent_sequence(&mut self, sequence: FullSequence, status: ObjectStatus) {
        assert!(
            !self.max_object_ids.contains_key(&sequence.group_id)
                || self.max_object_ids[&sequence.group_id] < sequence.object_id
        );

        match status {
            ObjectStatus::Normal | ObjectStatus::ObjectDoesNotExist => {
                if self.next_sequence <= sequence {
                    self.next_sequence = sequence.next();
                }
            }
            ObjectStatus::GroupDoesNotExist => {
                self.max_object_ids.insert(sequence.group_id, 0);
            }
            ObjectStatus::EndOfGroup => {
                self.max_object_ids
                    .insert(sequence.group_id, sequence.object_id);
                if self.next_sequence <= sequence {
                    self.next_sequence = FullSequence {
                        group_id: sequence.group_id + 1,
                        object_id: 0,
                    };
                }
            }
            ObjectStatus::EndOfTrack => {
                self.max_object_ids
                    .insert(sequence.group_id, sequence.object_id);
            }
            _ => {
                error!("invalid object status");
            }
        }
    }

    pub fn has_subscriber(&self) -> bool {
        !self.windows.is_empty()
    }

    pub fn get_window(&self, subscribe_id: u64) -> Option<&SubscribeWindow> {
        self.windows.get_window(subscribe_id)
    }

    pub fn forwarding_preference(&self) -> ObjectForwardingPreference {
        self.forwarding_preference
    }

    pub fn set_announce_cancel(&mut self) {
        self.announce_canceled = true;
    }
    pub fn canceled(&self) -> bool {
        self.announce_canceled
    }
}
