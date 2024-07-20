use crate::message::object::{ObjectForwardingPreference, ObjectStatus};
use crate::message::FullSequence;
use crate::StreamId;
use log::error;
use std::collections::HashMap;

/// Classes to track subscriptions to local tracks: the sequence numbers
/// subscribed, the streams involved, and the subscribe IDs.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct SubscribeWindow {
    subscribe_id: u64,
    start: FullSequence,
    end: Option<FullSequence>,
    largest_delivered: Option<FullSequence>,
    // The next sequence number to be redelivered, because it was published prior
    // to the subscription. Is none if no redeliveries are needed.
    next_to_backfill: Option<FullSequence>,
    // The first unpublished sequence number when the subscriber arrived.
    original_next_object: FullSequence,
    // Store open streams for this subscription. If the forwarding preference is
    // kTrack, there is one entry under sequence (0, 0). If kGroup, each entry is
    // under (group, 0). If kObject, it's tracked under the full sequence. If
    // kDatagram, the map is empty.
    send_streams: HashMap<FullSequence, StreamId>,
    // The forwarding preference for this track; informs how the streams are
    // mapped.
    forwarding_preference: ObjectForwardingPreference,
}

impl SubscribeWindow {
    pub fn new(
        subscribe_id: u64,
        forwarding_preference: ObjectForwardingPreference,
        next_object: FullSequence,
        start: FullSequence,
        end: Option<FullSequence>,
    ) -> Self {
        Self {
            subscribe_id,
            start,
            end,
            largest_delivered: None,
            next_to_backfill: if start < next_object {
                Some(start)
            } else {
                None
            },
            original_next_object: next_object,
            send_streams: Default::default(),
            forwarding_preference,
        }
    }

    pub fn subscribe_id(&self) -> u64 {
        self.subscribe_id
    }

    pub fn in_window(&self, seq: FullSequence) -> bool {
        if seq < self.start {
            return false;
        }

        if let Some(end) = self.end.as_ref() {
            seq <= *end
        } else {
            true
        }
    }

    /// Returns the stream to send |sequence| on, if already opened.
    pub fn get_stream_for_sequence(&self, seq: FullSequence) -> Option<&StreamId> {
        let index = self.sequence_to_index(seq);
        self.send_streams.get(&index)
    }

    /// Records what stream is being used for a track, group, or object depending
    /// on |forwarding_preference|. Triggers QUIC_BUG if already assigned.
    pub fn add_stream(&mut self, group_id: u64, object_id: u64, stream_id: StreamId) {
        if !self.in_window(FullSequence {
            group_id,
            object_id,
        }) {
            return;
        }
        let index = self.sequence_to_index(FullSequence {
            group_id,
            object_id,
        });
        if self.forwarding_preference == ObjectForwardingPreference::Datagram {
            error!("Adding a stream for datagram");
            return;
        }
        if self.send_streams.contains_key(&index) {
            error!("Stream already added");
            return;
        }
        self.send_streams.insert(index, stream_id);
    }

    pub fn remove_stream(&mut self, group_id: u64, object_id: u64) {
        let index = self.sequence_to_index(FullSequence {
            group_id,
            object_id,
        });
        self.send_streams.remove(&index);
    }

    pub fn has_end(&self) -> bool {
        self.end.is_some()
    }

    pub fn forwarding_preference(&self) -> ObjectForwardingPreference {
        self.forwarding_preference
    }

    /// Returns true if the object delivery completed the subscription
    pub fn on_object_sent(&mut self, sequence: FullSequence, status: ObjectStatus) -> bool {
        let update = if let Some(largest_delivered) = self.largest_delivered.as_ref() {
            *largest_delivered < sequence
        } else {
            true
        };
        if update {
            self.largest_delivered = Some(sequence);
        }

        // Update next_to_backfill_
        if sequence < self.original_next_object {
            if let Some(next_to_backfill) = self.next_to_backfill.as_ref() {
                if *next_to_backfill <= sequence {
                    match status {
                        ObjectStatus::Normal | ObjectStatus::ObjectDoesNotExist => {
                            self.next_to_backfill = Some(sequence.next());
                        }
                        ObjectStatus::EndOfGroup => {
                            self.next_to_backfill = Some(FullSequence {
                                group_id: sequence.group_id + 1,
                                object_id: 0,
                            });
                        }
                        _ => {
                            // Includes kEndOfTrack.
                            self.next_to_backfill = None;
                        }
                    }
                }
            }

            if let Some(next_to_backfill) = self.next_to_backfill.as_ref() {
                if *next_to_backfill == self.original_next_object
                    || (self.end.is_some() && *next_to_backfill == *self.end.as_ref().unwrap())
                {
                    self.next_to_backfill = None;
                }
            }
        }

        self.next_to_backfill.is_none()
            && self.end.is_some()
            && *self.end.as_ref().unwrap() <= sequence
    }

    pub fn largest_delivered(&self) -> Option<FullSequence> {
        self.largest_delivered
    }

    /// Returns true if the updated values are valid.
    pub fn update_start_end(&mut self, start: FullSequence, end: Option<FullSequence>) -> bool {
        // Can't make the subscription window bigger.
        if !self.in_window(start) {
            return false;
        }
        if let Some(old_end) = self.end.as_ref() {
            if let Some(new_end) = end.as_ref() {
                if *old_end < *new_end {
                    return false;
                }
            } else {
                return false;
            }
        }

        self.start = start;
        self.end = end;
        true
    }

    // Converts an object sequence number into one that matches the way that
    // stream IDs are being mapped. (See the comment for send_streams_ below.)
    fn sequence_to_index(&self, sequence: FullSequence) -> FullSequence {
        match self.forwarding_preference {
            ObjectForwardingPreference::Track => FullSequence {
                group_id: 0,
                object_id: 0,
            },
            ObjectForwardingPreference::Group => FullSequence {
                group_id: sequence.group_id,
                object_id: 0,
            },
            ObjectForwardingPreference::Object => sequence,
            ObjectForwardingPreference::Datagram => {
                error!("No stream for datagram");
                FullSequence {
                    group_id: 0,
                    object_id: 0,
                }
            }
        }
    }
}

pub struct SubscribeWindows {
    windows: HashMap<u64, SubscribeWindow>,
    forwarding_preference: ObjectForwardingPreference,
}

impl SubscribeWindows {
    pub fn new(forwarding_preference: ObjectForwardingPreference) -> Self {
        Self {
            windows: HashMap::new(),
            forwarding_preference,
        }
    }

    /// Returns a vector of subscribe IDs that apply to the object. They will be in
    /// reverse order of the add_window calls.
    pub fn sequence_is_subscribed(&self, sequence: FullSequence) -> Vec<&SubscribeWindow> {
        let mut retval = vec![];

        for window in self.windows.values() {
            if window.in_window(sequence) {
                retval.push(window)
            }
        }

        retval
    }

    /// |start_group| and |start_object| must be absolute sequence numbers. An
    /// optimization could consolidate overlapping subscribe windows.
    pub fn add_window(
        &mut self,
        subscribe_id: u64,
        next_object: FullSequence,
        start: FullSequence,
        end: Option<FullSequence>,
    ) {
        self.windows.insert(
            subscribe_id,
            SubscribeWindow::new(
                subscribe_id,
                self.forwarding_preference,
                next_object,
                start,
                end,
            ),
        );
    }

    pub fn remove_window(&mut self, subscribe_id: u64) {
        self.windows.remove(&subscribe_id);
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn get_window(&self, subscribe_id: u64) -> Option<&SubscribeWindow> {
        self.windows.get(&subscribe_id)
    }
}
