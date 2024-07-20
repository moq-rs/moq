use crate::message::object::{ObjectForwardingPreference, ObjectStatus};
use crate::message::FullSequence;
use crate::{Error, Result, StreamId};
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
    pub fn add_stream(&mut self, group_id: u64, object_id: u64, stream_id: StreamId) -> Result<()> {
        if !self.in_window(FullSequence {
            group_id,
            object_id,
        }) {
            return Ok(());
        }
        let index = self.sequence_to_index(FullSequence {
            group_id,
            object_id,
        });
        if self.forwarding_preference == ObjectForwardingPreference::Datagram {
            return Err(Error::ErrOther("Adding a stream for datagram".to_string()));
        }
        if self.send_streams.contains_key(&index) {
            return Err(Error::ErrOther("Stream already added".to_string()));
        }
        self.send_streams.insert(index, stream_id);
        Ok(())
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::Result;

    struct SubscribeWindowTest {
        subscribe_id: u64,
        right_edge: FullSequence,
        start: FullSequence,
        end: FullSequence,
    }

    impl SubscribeWindowTest {
        fn new() -> Self {
            Self {
                subscribe_id: 2,
                right_edge: FullSequence::new(4, 5),
                start: FullSequence::new(4, 0),
                end: FullSequence::new(5, 5),
            }
        }
    }

    #[test]
    fn test_subscribe_window_test_queries() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            test.right_edge,
            test.start,
            Some(test.end),
        );
        assert_eq!(window.subscribe_id(), 2);
        assert!(window.in_window(FullSequence::new(4, 0)));
        assert!(window.in_window(FullSequence::new(5, 5)));
        assert!(!window.in_window(FullSequence::new(5, 6)));
        assert!(!window.in_window(FullSequence::new(6, 0)));
        assert!(!window.in_window(FullSequence::new(3, 12)));
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_add_query_remove_stream_id_track() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Track,
            test.right_edge,
            test.start,
            Some(test.end),
        );
        assert!(window.add_stream(4, 0, 2).is_ok());
        assert_eq!(
            Error::ErrOther("Stream already added".to_string()),
            window.add_stream(5, 2, 6).unwrap_err()
        );
        assert_eq!(
            window.get_stream_for_sequence(FullSequence::new(5, 2)),
            Some(2).as_ref()
        );
        window.remove_stream(7, 2);
        assert!(!window
            .get_stream_for_sequence(FullSequence::new(4, 0))
            .is_some());
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_add_query_remove_stream_id_group() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Group,
            test.right_edge,
            test.start,
            Some(test.end),
        );
        assert!(window.add_stream(4, 0, 2).is_ok());
        assert!(!window
            .get_stream_for_sequence(FullSequence::new(5, 0))
            .is_some());
        assert!(window.add_stream(5, 2, 6).is_ok());
        assert_eq!(
            Error::ErrOther("Stream already added".to_string()),
            window.add_stream(5, 3, 6).unwrap_err()
        );
        assert_eq!(
            window.get_stream_for_sequence(FullSequence::new(4, 1)),
            Some(2).as_ref()
        );
        assert_eq!(
            window.get_stream_for_sequence(FullSequence::new(5, 0)),
            Some(6).as_ref()
        );
        window.remove_stream(5, 1);
        assert!(!window
            .get_stream_for_sequence(FullSequence::new(5, 2))
            .is_some());
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_add_query_remove_stream_id_object() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            test.right_edge,
            test.start,
            Some(test.end),
        );
        assert!(window.add_stream(4, 0, 2).is_ok());
        assert!(window.add_stream(4, 1, 6).is_ok());
        assert!(window.add_stream(4, 2, 10).is_ok());
        assert_eq!(
            window.add_stream(4, 2, 14).unwrap_err(),
            Error::ErrOther("Stream already added".to_string())
        );
        assert_eq!(
            window.get_stream_for_sequence(FullSequence::new(4, 0)),
            Some(2).as_ref()
        );
        assert_eq!(
            window.get_stream_for_sequence(FullSequence::new(4, 2)),
            Some(10).as_ref()
        );
        assert!(!window
            .get_stream_for_sequence(FullSequence::new(4, 4))
            .is_some());
        assert!(!window
            .get_stream_for_sequence(FullSequence::new(5, 0))
            .is_some());
        window.remove_stream(4, 2);
        assert!(!window
            .get_stream_for_sequence(FullSequence::new(4, 2))
            .is_some());
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_add_query_remove_stream_id_datagram() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Datagram,
            test.right_edge,
            test.start,
            Some(test.end),
        );
        assert_eq!(
            window.add_stream(4, 0, 2).unwrap_err(),
            Error::ErrOther("Adding a stream for datagram".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_on_object_sent() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            test.right_edge,
            test.start,
            Some(test.end),
        );
        assert!(!window.largest_delivered().is_some());
        assert!(!window.on_object_sent(FullSequence::new(4, 1), ObjectStatus::Normal));
        assert!(window.largest_delivered().is_some());
        assert_eq!(window.largest_delivered().unwrap(), FullSequence::new(4, 1));
        assert!(!window.on_object_sent(FullSequence::new(4, 2), ObjectStatus::Normal));
        assert_eq!(window.largest_delivered().unwrap(), FullSequence::new(4, 2));
        assert!(!window.on_object_sent(FullSequence::new(4, 0), ObjectStatus::Normal));
        assert_eq!(window.largest_delivered().unwrap(), FullSequence::new(4, 2));
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_all_objects_unpublished_at_start() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            FullSequence::new(0, 0),
            FullSequence::new(0, 0),
            Some(FullSequence::new(0, 1)),
        );
        assert!(!window.on_object_sent(FullSequence::new(0, 0), ObjectStatus::Normal));
        assert!(window.on_object_sent(FullSequence::new(0, 1), ObjectStatus::Normal));
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_all_objects_published_at_start() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            FullSequence::new(4, 0),
            FullSequence::new(0, 0),
            Some(FullSequence::new(0, 1)),
        );
        assert!(!window.on_object_sent(FullSequence::new(0, 0), ObjectStatus::Normal));
        assert!(window.on_object_sent(FullSequence::new(0, 1), ObjectStatus::Normal));
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_some_objects_unpublished_at_start() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            FullSequence::new(0, 1),
            FullSequence::new(0, 0),
            Some(FullSequence::new(0, 1)),
        );
        assert!(!window.on_object_sent(FullSequence::new(0, 0), ObjectStatus::Normal));
        assert!(window.on_object_sent(FullSequence::new(0, 1), ObjectStatus::Normal));
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_update_start_end() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            test.right_edge,
            test.start,
            Some(test.end),
        );
        assert!(window.update_start_end(
            test.start.next(),
            Some(FullSequence::new(test.end.group_id, test.end.object_id - 1)),
        ));
        assert!(!window.in_window(FullSequence::new(test.start.group_id, test.start.object_id)));
        assert!(!window.in_window(FullSequence::new(test.end.group_id, test.end.object_id)));
        assert!(!window.update_start_end(
            test.start,
            Some(FullSequence::new(test.end.group_id, test.end.object_id - 1)),
        ));
        assert!(!window.update_start_end(test.start.next(), Some(test.end)));
        Ok(())
    }

    #[test]
    fn test_subscribe_window_test_update_start_end_open_ended() -> Result<()> {
        let test = SubscribeWindowTest::new();
        let mut window = SubscribeWindow::new(
            test.subscribe_id,
            ObjectForwardingPreference::Object,
            test.right_edge,
            test.start,
            None,
        );
        assert!(window.update_start_end(test.start, Some(test.end)));
        assert!(!window.in_window(test.end.next()));
        assert!(!window.update_start_end(test.start, None));
        Ok(())
    }

    struct SubscribeWindowsTest {
        windows: SubscribeWindows,
    }

    impl SubscribeWindowsTest {
        fn new() -> Self {
            Self {
                windows: SubscribeWindows::new(ObjectForwardingPreference::Object),
            }
        }
    }

    #[test]
    fn test_moqt_subscribe_windows_test_is_empty() -> Result<()> {
        let windows = &mut SubscribeWindowsTest::new().windows;
        assert!(windows.is_empty());
        windows.add_window(0, FullSequence::new(2, 1), FullSequence::new(1, 3), None);
        assert!(!windows.is_empty());
        Ok(())
    }

    #[test]
    fn test_moqt_subscribe_windows_test_is_subscribed() -> Result<()> {
        let windows = &mut SubscribeWindowsTest::new().windows;
        assert!(windows.is_empty());
        // The first two windows overlap; the third is open-ended.
        windows.add_window(
            0,
            FullSequence::new(0, 0),
            FullSequence::new(1, 0),
            Some(FullSequence::new(3, 9)),
        );
        windows.add_window(
            1,
            FullSequence::new(0, 0),
            FullSequence::new(2, 4),
            Some(FullSequence::new(4, 3)),
        );
        windows.add_window(2, FullSequence::new(0, 0), FullSequence::new(10, 0), None);
        assert!(!windows.is_empty());
        assert!(windows
            .sequence_is_subscribed(FullSequence::new(0, 8))
            .is_empty());
        let mut hits = windows.sequence_is_subscribed(FullSequence::new(1, 0));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].subscribe_id(), 0);
        assert!(windows
            .sequence_is_subscribed(FullSequence::new(4, 4))
            .is_empty());
        assert!(windows
            .sequence_is_subscribed(FullSequence::new(8, 3))
            .is_empty());
        hits = windows.sequence_is_subscribed(FullSequence::new(100, 7));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].subscribe_id(), 2);
        hits = windows.sequence_is_subscribed(FullSequence::new(3, 0));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].subscribe_id() + hits[1].subscribe_id(), 1);
        Ok(())
    }

    #[test]
    fn test_moqt_subscribe_windows_test_add_get_remove_window() -> Result<()> {
        let windows = &mut SubscribeWindowsTest::new().windows;
        windows.add_window(
            0,
            FullSequence::new(2, 5),
            FullSequence::new(1, 0),
            Some(FullSequence::new(3, 9)),
        );
        let window = windows.get_window(0).unwrap();
        assert_eq!(window.subscribe_id(), 0);
        assert_eq!(windows.get_window(1), None);
        windows.remove_window(0);
        assert_eq!(windows.get_window(0), None);
        Ok(())
    }
}
