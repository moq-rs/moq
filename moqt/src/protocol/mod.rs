use crate::message::client_setup::ClientSetup;
use crate::message::message_framer::MessageFramer;
use crate::message::message_parser::{MessageParser, MessageParserEvent};
use crate::message::object::ObjectHeader;
use crate::message::server_setup::ServerSetup;
use crate::message::subscribe::Subscribe;
use crate::message::subscribe_done::SubscribeDone;
use crate::message::subscribe_error::SubscribeError;
use crate::message::subscribe_ok::SubscribeOk;
use crate::message::subscribe_update::SubscribeUpdate;
use crate::message::unsubscribe::UnSubscribe;
use crate::message::{ControlMessage, FilterType, FullSequence, FullTrackName, Role, Version};
use crate::session::remote_track::{RemoteTrack, RemoteTrackOnObjectFragment};
use crate::{Result, StreamId};
use bytes::{Bytes, BytesMut};
use sansio::Protocol;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Perspective {
    #[default]
    Server,
    Client,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub(crate) struct Config {
    pub version: Version,
    pub perspective: Perspective,
    pub use_web_transport: bool,
    pub path: String,
    pub deliver_partial_objects: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum StreamPurpose {
    Control,
    Data,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ReadInput {
    StreamData {
        stream_id: StreamId,
        data: Bytes,
        fin: bool,
    },
    Datagram(Bytes),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ReadOutput {
    StreamData {
        stream_id: StreamId,
        data: Bytes,
        fin: bool,
    },
    Datagram(Bytes),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SessionState {
    AwaitingSetup,
    Established,
    Closed,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Subscription {
    full_track_name: FullTrackName,
    track_alias: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct IncomingSubscribe {
    subscription: Subscription,
    accepted: bool,
}

struct DataStreamState {
    parser: MessageParser,
    partial_object: Option<(ObjectHeader, BytesMut)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Command {
    Close {
        code: u64,
        reason: String,
    },
    Subscribe {
        track_namespace: String,
        track_name: String,
        filter_type: FilterType,
        authorization_info: Option<String>,
    },
    SubscribeOk {
        subscribe_id: u64,
        expires: u64,
        largest_group_object: Option<FullSequence>,
    },
    SubscribeError {
        subscribe_id: u64,
        error_code: u64,
        reason_phrase: String,
        track_alias: u64,
    },
    SubscribeUpdate {
        subscribe_id: u64,
        start_group_object: FullSequence,
        end_group_object: Option<FullSequence>,
        authorization_info: Option<String>,
    },
    SubscribeDone {
        subscribe_id: u64,
        status_code: u64,
        reason_phrase: String,
        final_group_object: Option<FullSequence>,
    },
    Unsubscribe {
        subscribe_id: u64,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum EventIn {
    TransportConnected,
    TransportClosed,
    StreamOpened {
        stream_id: StreamId,
        bidi: bool,
        local: bool,
    },
    StreamClosed {
        stream_id: StreamId,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum EventOut {
    SessionEstablished {
        peer_role: Option<Role>,
        path: Option<String>,
    },
    SubscribeReceived(Subscribe),
    SubscribeAccepted {
        subscribe_id: u64,
        full_track_name: FullTrackName,
        track_alias: u64,
        expires: u64,
        largest_group_object: Option<FullSequence>,
    },
    SubscribeRejected {
        subscribe_id: u64,
        full_track_name: FullTrackName,
        error_code: u64,
        reason_phrase: String,
        track_alias: u64,
    },
    SubscribeUpdated(SubscribeUpdate),
    SubscribeEnded {
        subscribe_id: u64,
        full_track_name: FullTrackName,
        track_alias: u64,
        status_code: u64,
        reason_phrase: String,
        final_group_object: Option<FullSequence>,
    },
    ObjectReceived {
        full_track_name: FullTrackName,
        fragment: RemoteTrackOnObjectFragment,
    },
    UnsubscribeReceived {
        subscribe_id: u64,
    },
    SessionTerminated,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum WriteOutput {
    OpenBiStream {
        purpose: StreamPurpose,
    },
    SendStream {
        stream_id: StreamId,
        bytes: BytesMut,
        fin: bool,
    },
    SendDatagram(Bytes),
    Close {
        code: u64,
        reason: String,
    },
}

/// New SANS-I/O session core skeleton.
///
/// This sits alongside the older `Handler`-based session code while the rewrite
/// is in progress. It currently covers only the earliest lifecycle needed to
/// bootstrap a client control stream and emit `CLIENT_SETUP`.
pub(crate) struct SessionCore {
    config: Config,
    state: SessionState,
    control_stream_id: Option<StreamId>,
    control_parser: Option<MessageParser>,
    remote_track_aliases: HashMap<FullTrackName, u64>,
    remote_tracks: HashMap<u64, RemoteTrack>,
    pending_outgoing_subscribes: HashMap<u64, Subscription>,
    active_outgoing_subscribes: HashMap<u64, Subscription>,
    incoming_subscribes: HashMap<u64, IncomingSubscribe>,
    data_streams: HashMap<StreamId, DataStreamState>,
    next_remote_track_alias: u64,
    next_subscribe_id: u64,
    routs: VecDeque<ReadOutput>,
    wouts: VecDeque<WriteOutput>,
    eouts: VecDeque<EventOut>,
}

impl SessionCore {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            state: SessionState::AwaitingSetup,
            control_stream_id: None,
            control_parser: None,
            remote_track_aliases: HashMap::new(),
            remote_tracks: HashMap::new(),
            pending_outgoing_subscribes: HashMap::new(),
            active_outgoing_subscribes: HashMap::new(),
            incoming_subscribes: HashMap::new(),
            data_streams: HashMap::new(),
            next_remote_track_alias: 0,
            next_subscribe_id: 0,
            routs: VecDeque::new(),
            wouts: VecDeque::new(),
            eouts: VecDeque::new(),
        }
    }

    fn close_with_protocol_violation(&mut self, reason: impl Into<String>) {
        self.wouts.push_back(WriteOutput::Close {
            code: 1,
            reason: reason.into(),
        });
    }

    fn send_control_message(&mut self, control_message: ControlMessage) -> Result<()> {
        let stream_id = self
            .control_stream_id
            .ok_or_else(|| crate::Error::ErrOther("control stream not established".to_string()))?;
        let mut bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(control_message, &mut bytes)?;
        self.wouts.push_back(WriteOutput::SendStream {
            stream_id,
            bytes,
            fin: false,
        });
        Ok(())
    }

    fn send_client_setup(&mut self, stream_id: StreamId) -> Result<()> {
        let mut client_setup = ClientSetup {
            supported_versions: vec![self.config.version],
            role: Some(Role::PubSub),
            path: None,
            uses_web_transport: self.config.use_web_transport,
        };
        if !self.config.use_web_transport {
            client_setup.path = Some(self.config.path.clone());
        }

        self.ensure_control_stream(stream_id);
        self.send_control_message(ControlMessage::ClientSetup(client_setup))?;
        Ok(())
    }

    fn send_server_setup(&mut self, stream_id: StreamId) -> Result<()> {
        let server_setup = ServerSetup {
            supported_version: self.config.version,
            role: Some(Role::PubSub),
        };

        self.ensure_control_stream(stream_id);
        self.send_control_message(ControlMessage::ServerSetup(server_setup))?;
        Ok(())
    }

    fn ensure_control_stream(&mut self, stream_id: StreamId) {
        if self.control_stream_id.is_none() {
            self.control_stream_id = Some(stream_id);
        }
        if self.control_parser.is_none() {
            self.control_parser = Some(MessageParser::new(self.config.use_web_transport));
        }
    }

    fn data_stream(&mut self, stream_id: StreamId) -> &mut DataStreamState {
        self.data_streams
            .entry(stream_id)
            .or_insert_with(|| DataStreamState {
                parser: MessageParser::new(self.config.use_web_transport),
                partial_object: None,
            })
    }

    fn on_object_message(
        &mut self,
        stream_id: StreamId,
        object_header: ObjectHeader,
        mut payload: Bytes,
        fin: bool,
    ) {
        let Some(subscription) = self
            .active_outgoing_subscribes
            .get(&object_header.subscribe_id)
        else {
            self.close_with_protocol_violation(format!(
                "received object for unknown subscribe_id {}",
                object_header.subscribe_id
            ));
            return;
        };
        if subscription.track_alias != object_header.track_alias {
            self.close_with_protocol_violation(format!(
                "received object for subscribe_id {} with unexpected track_alias {}",
                object_header.subscribe_id, object_header.track_alias
            ));
            return;
        }

        let full_track_name = {
            let remote_track = self
                .remote_tracks
                .entry(object_header.track_alias)
                .or_insert_with(|| {
                    RemoteTrack::new(
                        subscription.full_track_name.clone(),
                        subscription.track_alias,
                    )
                });
            if !remote_track.check_forwarding_preference(object_header.object_forwarding_preference)
            {
                self.close_with_protocol_violation(format!(
                    "inconsistent forwarding preference for track_alias {}",
                    object_header.track_alias
                ));
                return;
            }
            remote_track.full_track_name().clone()
        };

        if !self.config.deliver_partial_objects && !fin {
            let data_stream = self.data_stream(stream_id);
            if let Some((buffered_header, partial)) = data_stream.partial_object.as_mut() {
                if *buffered_header != object_header {
                    self.close_with_protocol_violation(
                        "received new partial object before previous object completed",
                    );
                    return;
                }
                partial.extend_from_slice(payload.as_ref());
            } else {
                let mut partial = BytesMut::new();
                partial.extend_from_slice(payload.as_ref());
                data_stream.partial_object = Some((object_header, partial));
            }
            return;
        }

        if !self.config.deliver_partial_objects {
            let data_stream = self.data_stream(stream_id);
            if let Some((buffered_header, mut partial)) = data_stream.partial_object.take() {
                if buffered_header != object_header {
                    self.close_with_protocol_violation(
                        "completed object header does not match buffered partial object",
                    );
                    return;
                }
                partial.extend_from_slice(payload.as_ref());
                payload = partial.freeze();
            }
        }

        self.eouts.push_back(EventOut::ObjectReceived {
            full_track_name,
            fragment: RemoteTrackOnObjectFragment {
                object_header,
                payload,
                fin,
            },
        });
    }

    fn process_stream_data(&mut self, stream_id: StreamId, data: Bytes, fin: bool) {
        let mut events = Vec::new();
        {
            let data_stream = self.data_stream(stream_id);
            data_stream.parser.process_data(&mut data.as_ref(), fin);
            while let Some(event) = data_stream.parser.poll_event() {
                events.push(event);
            }
        }

        for event in events {
            match event {
                MessageParserEvent::ControlMessage(control_message) => {
                    self.close_with_protocol_violation(format!(
                        "received control message on data stream: {:?}",
                        control_message
                    ));
                }
                MessageParserEvent::ParsingError(_, reason) => {
                    self.wouts.push_back(WriteOutput::Close { code: 1, reason });
                }
                MessageParserEvent::ObjectMessage(object_header, payload, fin) => {
                    self.on_object_message(stream_id, object_header, payload, fin);
                }
            }
        }
    }

    fn process_datagram(&mut self, bytes: Bytes) {
        let (object_header, payload) = match MessageParser::process_datagram(&mut bytes.as_ref()) {
            Ok(value) => value,
            Err(error) => {
                self.close_with_protocol_violation(error.to_string());
                return;
            }
        };
        self.on_object_message(0, object_header, payload, true);
    }

    fn on_control_message(&mut self, control_message: ControlMessage) -> Result<()> {
        match control_message {
            ControlMessage::ClientSetup(client_setup) => {
                if self.state != SessionState::AwaitingSetup {
                    self.close_with_protocol_violation("received duplicate CLIENT_SETUP");
                    return Ok(());
                }
                if self.config.perspective != Perspective::Server {
                    self.close_with_protocol_violation("received CLIENT_SETUP as client");
                    return Ok(());
                }
                if !client_setup
                    .supported_versions
                    .contains(&self.config.version)
                {
                    self.close_with_protocol_violation(format!(
                        "version mismatch: expected {:?}",
                        self.config.version
                    ));
                    return Ok(());
                }
                let stream_id = self.control_stream_id.expect("control stream set");
                self.send_server_setup(stream_id)?;
                self.state = SessionState::Established;
                self.eouts.push_back(EventOut::SessionEstablished {
                    peer_role: client_setup.role,
                    path: client_setup.path,
                });
            }
            ControlMessage::ServerSetup(server_setup) => {
                if self.state != SessionState::AwaitingSetup {
                    self.close_with_protocol_violation("received duplicate SERVER_SETUP");
                    return Ok(());
                }
                if self.config.perspective != Perspective::Client {
                    self.close_with_protocol_violation("received SERVER_SETUP as server");
                    return Ok(());
                }
                if server_setup.supported_version != self.config.version {
                    self.close_with_protocol_violation(format!(
                        "version mismatch: expected {:?}",
                        self.config.version
                    ));
                    return Ok(());
                }
                self.state = SessionState::Established;
                self.eouts.push_back(EventOut::SessionEstablished {
                    peer_role: server_setup.role,
                    path: None,
                });
            }
            ControlMessage::Subscribe(subscribe) => {
                if self.state != SessionState::Established {
                    self.close_with_protocol_violation("received SUBSCRIBE before session setup");
                    return Ok(());
                }
                if self
                    .incoming_subscribes
                    .contains_key(&subscribe.subscribe_id)
                {
                    self.close_with_protocol_violation(format!(
                        "received duplicate SUBSCRIBE for subscribe_id {}",
                        subscribe.subscribe_id
                    ));
                    return Ok(());
                }
                self.incoming_subscribes.insert(
                    subscribe.subscribe_id,
                    IncomingSubscribe {
                        subscription: Subscription {
                            full_track_name: FullTrackName::new(
                                subscribe.track_namespace.clone(),
                                subscribe.track_name.clone(),
                            ),
                            track_alias: subscribe.track_alias,
                        },
                        accepted: false,
                    },
                );
                self.eouts.push_back(EventOut::SubscribeReceived(subscribe));
            }
            ControlMessage::SubscribeOk(subscribe_ok) => {
                if self.state != SessionState::Established {
                    self.close_with_protocol_violation(
                        "received SUBSCRIBE_OK before session setup",
                    );
                    return Ok(());
                }
                let Some(subscription) = self
                    .pending_outgoing_subscribes
                    .remove(&subscribe_ok.subscribe_id)
                else {
                    self.close_with_protocol_violation(format!(
                        "received SUBSCRIBE_OK for unknown subscribe_id {}",
                        subscribe_ok.subscribe_id
                    ));
                    return Ok(());
                };
                self.active_outgoing_subscribes
                    .insert(subscribe_ok.subscribe_id, subscription.clone());
                self.remote_tracks
                    .entry(subscription.track_alias)
                    .or_insert_with(|| {
                        RemoteTrack::new(
                            subscription.full_track_name.clone(),
                            subscription.track_alias,
                        )
                    });
                self.eouts.push_back(EventOut::SubscribeAccepted {
                    subscribe_id: subscribe_ok.subscribe_id,
                    full_track_name: subscription.full_track_name,
                    track_alias: subscription.track_alias,
                    expires: subscribe_ok.expires,
                    largest_group_object: subscribe_ok.largest_group_object,
                });
            }
            ControlMessage::SubscribeError(subscribe_error) => {
                if self.state != SessionState::Established {
                    self.close_with_protocol_violation(
                        "received SUBSCRIBE_ERROR before session setup",
                    );
                    return Ok(());
                }
                let Some(subscription) = self
                    .pending_outgoing_subscribes
                    .remove(&subscribe_error.subscribe_id)
                else {
                    self.close_with_protocol_violation(format!(
                        "received SUBSCRIBE_ERROR for unknown subscribe_id {}",
                        subscribe_error.subscribe_id
                    ));
                    return Ok(());
                };
                self.eouts.push_back(EventOut::SubscribeRejected {
                    subscribe_id: subscribe_error.subscribe_id,
                    full_track_name: subscription.full_track_name,
                    error_code: subscribe_error.error_code,
                    reason_phrase: subscribe_error.reason_phrase,
                    track_alias: subscribe_error.track_alias,
                });
            }
            ControlMessage::SubscribeUpdate(subscribe_update) => {
                if self.state != SessionState::Established {
                    self.close_with_protocol_violation(
                        "received SUBSCRIBE_UPDATE before session setup",
                    );
                    return Ok(());
                }
                let Some(incoming_subscribe) =
                    self.incoming_subscribes.get(&subscribe_update.subscribe_id)
                else {
                    self.close_with_protocol_violation(format!(
                        "received SUBSCRIBE_UPDATE for unknown subscribe_id {}",
                        subscribe_update.subscribe_id
                    ));
                    return Ok(());
                };
                if !incoming_subscribe.accepted {
                    self.close_with_protocol_violation(format!(
                        "received SUBSCRIBE_UPDATE before SUBSCRIBE_OK for subscribe_id {}",
                        subscribe_update.subscribe_id
                    ));
                    return Ok(());
                }
                self.eouts
                    .push_back(EventOut::SubscribeUpdated(subscribe_update));
            }
            ControlMessage::SubscribeDone(subscribe_done) => {
                if self.state != SessionState::Established {
                    self.close_with_protocol_violation(
                        "received SUBSCRIBE_DONE before session setup",
                    );
                    return Ok(());
                }
                let Some(subscription) = self
                    .active_outgoing_subscribes
                    .remove(&subscribe_done.subscribe_id)
                else {
                    self.close_with_protocol_violation(format!(
                        "received SUBSCRIBE_DONE for unknown subscribe_id {}",
                        subscribe_done.subscribe_id
                    ));
                    return Ok(());
                };
                self.remote_tracks.remove(&subscription.track_alias);
                self.eouts.push_back(EventOut::SubscribeEnded {
                    subscribe_id: subscribe_done.subscribe_id,
                    full_track_name: subscription.full_track_name,
                    track_alias: subscription.track_alias,
                    status_code: subscribe_done.status_code,
                    reason_phrase: subscribe_done.reason_phrase,
                    final_group_object: subscribe_done.final_group_object,
                });
            }
            ControlMessage::UnSubscribe(unsubscribe) => {
                if self.state != SessionState::Established {
                    self.close_with_protocol_violation("received UNSUBSCRIBE before session setup");
                    return Ok(());
                }
                if self
                    .incoming_subscribes
                    .remove(&unsubscribe.subscribe_id)
                    .is_none()
                {
                    self.close_with_protocol_violation(format!(
                        "received UNSUBSCRIBE for unknown subscribe_id {}",
                        unsubscribe.subscribe_id
                    ));
                    return Ok(());
                }
                self.eouts.push_back(EventOut::UnsubscribeReceived {
                    subscribe_id: unsubscribe.subscribe_id,
                });
            }
            other => {
                self.close_with_protocol_violation(format!(
                    "unsupported control message: {:?}",
                    other
                ));
            }
        }
        Ok(())
    }
}

impl Protocol<ReadInput, Command, EventIn> for SessionCore {
    type Rout = ReadOutput;
    type Wout = WriteOutput;
    type Eout = EventOut;
    type Error = crate::Error;
    type Time = Instant;

    fn handle_read(&mut self, msg: ReadInput) -> Result<()> {
        match msg {
            ReadInput::StreamData {
                stream_id,
                data,
                fin,
            } => {
                if self.control_stream_id.is_none() || self.control_stream_id == Some(stream_id) {
                    self.ensure_control_stream(stream_id);
                    let mut events = Vec::new();
                    let parser = self.control_parser.as_mut().expect("control parser set");
                    parser.process_data(&mut data.as_ref(), fin);
                    while let Some(event) = parser.poll_event() {
                        events.push(event);
                    }
                    for event in events {
                        match event {
                            MessageParserEvent::ControlMessage(control_message) => {
                                self.on_control_message(control_message)?;
                            }
                            MessageParserEvent::ParsingError(_, reason) => {
                                self.wouts.push_back(WriteOutput::Close { code: 1, reason });
                            }
                            MessageParserEvent::ObjectMessage(_, _, _) => {
                                self.wouts.push_back(WriteOutput::Close {
                                    code: 1,
                                    reason: "received object on control stream".to_string(),
                                });
                            }
                        }
                    }
                } else {
                    self.process_stream_data(stream_id, data, fin);
                }
            }
            ReadInput::Datagram(bytes) => self.process_datagram(bytes),
        }
        Ok(())
    }

    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.routs.pop_front()
    }

    fn handle_write(&mut self, msg: Command) -> Result<()> {
        match msg {
            Command::Close { code, reason } => {
                self.wouts.push_back(WriteOutput::Close { code, reason });
            }
            Command::Subscribe {
                track_namespace,
                track_name,
                filter_type,
                authorization_info,
            } => {
                if self.state != SessionState::Established {
                    return Err(crate::Error::ErrOther(
                        "cannot send SUBSCRIBE before session established".to_string(),
                    ));
                }
                let full_track_name = FullTrackName::new(track_namespace, track_name);
                let track_alias =
                    if let Some(track_alias) = self.remote_track_aliases.get(&full_track_name) {
                        *track_alias
                    } else {
                        let track_alias = self.next_remote_track_alias;
                        self.next_remote_track_alias += 1;
                        self.remote_track_aliases
                            .insert(full_track_name.clone(), track_alias);
                        track_alias
                    };
                let subscribe_id = self.next_subscribe_id;
                self.next_subscribe_id += 1;
                let subscribe = Subscribe {
                    subscribe_id,
                    track_alias,
                    track_namespace: full_track_name.track_namespace.clone(),
                    track_name: full_track_name.track_name.clone(),
                    filter_type,
                    authorization_info,
                };
                self.send_control_message(ControlMessage::Subscribe(subscribe))?;
                self.pending_outgoing_subscribes.insert(
                    subscribe_id,
                    Subscription {
                        full_track_name,
                        track_alias,
                    },
                );
            }
            Command::SubscribeOk {
                subscribe_id,
                expires,
                largest_group_object,
            } => {
                if self.state != SessionState::Established {
                    return Err(crate::Error::ErrOther(
                        "cannot send SUBSCRIBE_OK before session established".to_string(),
                    ));
                }
                let Some(incoming_subscribe) = self.incoming_subscribes.get_mut(&subscribe_id)
                else {
                    return Err(crate::Error::ErrOther(format!(
                        "cannot send SUBSCRIBE_OK for unknown subscribe_id {}",
                        subscribe_id
                    )));
                };
                if incoming_subscribe.accepted {
                    return Err(crate::Error::ErrOther(format!(
                        "cannot send duplicate SUBSCRIBE_OK for subscribe_id {}",
                        subscribe_id
                    )));
                }
                incoming_subscribe.accepted = true;
                self.send_control_message(ControlMessage::SubscribeOk(SubscribeOk {
                    subscribe_id,
                    expires,
                    largest_group_object,
                }))?;
            }
            Command::SubscribeError {
                subscribe_id,
                error_code,
                reason_phrase,
                track_alias,
            } => {
                if self.state != SessionState::Established {
                    return Err(crate::Error::ErrOther(
                        "cannot send SUBSCRIBE_ERROR before session established".to_string(),
                    ));
                }
                if self.incoming_subscribes.remove(&subscribe_id).is_none() {
                    return Err(crate::Error::ErrOther(format!(
                        "cannot send SUBSCRIBE_ERROR for unknown subscribe_id {}",
                        subscribe_id
                    )));
                }
                self.send_control_message(ControlMessage::SubscribeError(SubscribeError {
                    subscribe_id,
                    error_code,
                    reason_phrase,
                    track_alias,
                }))?;
            }
            Command::SubscribeUpdate {
                subscribe_id,
                start_group_object,
                end_group_object,
                authorization_info,
            } => {
                if self.state != SessionState::Established {
                    return Err(crate::Error::ErrOther(
                        "cannot send SUBSCRIBE_UPDATE before session established".to_string(),
                    ));
                }
                if !self.active_outgoing_subscribes.contains_key(&subscribe_id) {
                    return Err(crate::Error::ErrOther(format!(
                        "cannot send SUBSCRIBE_UPDATE for unknown subscribe_id {}",
                        subscribe_id
                    )));
                }
                self.send_control_message(ControlMessage::SubscribeUpdate(SubscribeUpdate {
                    subscribe_id,
                    start_group_object,
                    end_group_object,
                    authorization_info,
                }))?;
            }
            Command::SubscribeDone {
                subscribe_id,
                status_code,
                reason_phrase,
                final_group_object,
            } => {
                if self.state != SessionState::Established {
                    return Err(crate::Error::ErrOther(
                        "cannot send SUBSCRIBE_DONE before session established".to_string(),
                    ));
                }
                let Some(incoming_subscribe) = self.incoming_subscribes.get(&subscribe_id) else {
                    return Err(crate::Error::ErrOther(format!(
                        "cannot send SUBSCRIBE_DONE for unknown subscribe_id {}",
                        subscribe_id
                    )));
                };
                if !incoming_subscribe.accepted {
                    return Err(crate::Error::ErrOther(format!(
                        "cannot send SUBSCRIBE_DONE before SUBSCRIBE_OK for subscribe_id {}",
                        subscribe_id
                    )));
                }
                self.incoming_subscribes.remove(&subscribe_id);
                self.send_control_message(ControlMessage::SubscribeDone(SubscribeDone {
                    subscribe_id,
                    status_code,
                    reason_phrase,
                    final_group_object,
                }))?;
            }
            Command::Unsubscribe { subscribe_id } => {
                if self.state != SessionState::Established {
                    return Err(crate::Error::ErrOther(
                        "cannot send UNSUBSCRIBE before session established".to_string(),
                    ));
                }
                if !self.pending_outgoing_subscribes.contains_key(&subscribe_id)
                    && !self.active_outgoing_subscribes.contains_key(&subscribe_id)
                {
                    return Err(crate::Error::ErrOther(format!(
                        "cannot send UNSUBSCRIBE for unknown subscribe_id {}",
                        subscribe_id
                    )));
                }
                self.send_control_message(ControlMessage::UnSubscribe(UnSubscribe {
                    subscribe_id,
                }))?;
            }
        }
        Ok(())
    }

    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.wouts.pop_front()
    }

    fn handle_event(&mut self, evt: EventIn) -> Result<()> {
        match evt {
            EventIn::TransportConnected => {
                if self.config.perspective == Perspective::Client
                    && self.control_stream_id.is_none()
                {
                    self.wouts.push_back(WriteOutput::OpenBiStream {
                        purpose: StreamPurpose::Control,
                    });
                }
            }
            EventIn::TransportClosed => {
                self.state = SessionState::Closed;
                self.control_stream_id = None;
                self.control_parser = None;
                self.remote_tracks.clear();
                self.pending_outgoing_subscribes.clear();
                self.active_outgoing_subscribes.clear();
                self.incoming_subscribes.clear();
                self.data_streams.clear();
                self.eouts.push_back(EventOut::SessionTerminated);
            }
            EventIn::StreamOpened {
                stream_id,
                bidi,
                local,
            } => {
                if self.config.perspective == Perspective::Client
                    && local
                    && bidi
                    && self.control_stream_id.is_none()
                {
                    self.ensure_control_stream(stream_id);
                    self.send_client_setup(stream_id)?;
                }
            }
            EventIn::StreamClosed { stream_id } => {
                if self.control_stream_id == Some(stream_id) {
                    self.control_stream_id = None;
                    self.control_parser = None;
                } else {
                    self.data_streams.remove(&stream_id);
                }
            }
        }
        Ok(())
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.eouts.pop_front()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::message_parser::MessageParser;
    use crate::message::object::{ObjectForwardingPreference, ObjectStatus};

    fn client_config(use_web_transport: bool) -> Config {
        Config {
            version: Version::Draft04,
            perspective: Perspective::Client,
            use_web_transport,
            path: "/moq".to_string(),
            deliver_partial_objects: false,
        }
    }

    fn server_config(use_web_transport: bool) -> Config {
        Config {
            version: Version::Draft04,
            perspective: Perspective::Server,
            use_web_transport,
            path: "/moq".to_string(),
            deliver_partial_objects: false,
        }
    }

    #[test]
    fn client_transport_connected_opens_control_stream() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(true));

        protocol.handle_event(EventIn::TransportConnected)?;

        assert_eq!(
            protocol.poll_write(),
            Some(WriteOutput::OpenBiStream {
                purpose: StreamPurpose::Control
            })
        );
        assert_eq!(protocol.poll_write(), None);
        Ok(())
    }

    #[test]
    fn client_stream_opened_sends_client_setup_for_webtransport() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(true));

        protocol.handle_event(EventIn::StreamOpened {
            stream_id: 7,
            bidi: true,
            local: true,
        })?;

        let Some(WriteOutput::SendStream {
            stream_id,
            bytes,
            fin,
        }) = protocol.poll_write()
        else {
            panic!("expected setup bytes");
        };
        assert_eq!(stream_id, 7);
        assert!(!fin);

        let mut parser = MessageParser::new(true);
        parser.process_data(&mut bytes.as_ref(), false);
        let event = parser.poll_event().expect("control event");
        match event {
            crate::message::message_parser::MessageParserEvent::ControlMessage(
                ControlMessage::ClientSetup(client_setup),
            ) => {
                assert_eq!(client_setup.supported_versions, vec![Version::Draft04]);
                assert_eq!(client_setup.role, Some(Role::PubSub));
                assert_eq!(client_setup.path, None);
                assert!(client_setup.uses_web_transport);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn client_stream_opened_sends_client_setup_for_raw_quic() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));

        protocol.handle_event(EventIn::StreamOpened {
            stream_id: 9,
            bidi: true,
            local: true,
        })?;

        let Some(WriteOutput::SendStream { bytes, fin, .. }) = protocol.poll_write() else {
            panic!("expected setup bytes");
        };
        assert!(!fin);

        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        let event = parser.poll_event().expect("control event");
        match event {
            crate::message::message_parser::MessageParserEvent::ControlMessage(
                ControlMessage::ClientSetup(client_setup),
            ) => {
                assert_eq!(client_setup.path, Some("/moq".to_string()));
                assert!(!client_setup.uses_web_transport);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn server_receives_client_setup_and_replies_with_server_setup() -> Result<()> {
        let mut client_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            }),
            &mut client_setup_bytes,
        )?;

        let mut protocol = SessionCore::new(server_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 11,
            data: client_setup_bytes.freeze(),
            fin: true,
        })?;

        let Some(WriteOutput::SendStream {
            stream_id, bytes, ..
        }) = protocol.poll_write()
        else {
            panic!("expected server setup response");
        };
        assert_eq!(stream_id, 11);

        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        let event = parser.poll_event().expect("control event");
        match event {
            MessageParserEvent::ControlMessage(ControlMessage::ServerSetup(server_setup)) => {
                assert_eq!(server_setup.supported_version, Version::Draft04);
                assert_eq!(server_setup.role, Some(Role::PubSub));
            }
            _ => panic!("unexpected parser event"),
        }

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SessionEstablished {
                peer_role: Some(Role::PubSub),
                path: Some("/moq".to_string())
            })
        );
        Ok(())
    }

    #[test]
    fn client_receives_server_setup_and_establishes_session() -> Result<()> {
        let mut server_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ServerSetup(ServerSetup {
                supported_version: Version::Draft04,
                role: Some(Role::PubSub),
            }),
            &mut server_setup_bytes,
        )?;

        let mut protocol = SessionCore::new(client_config(true));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 13,
            data: server_setup_bytes.freeze(),
            fin: true,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SessionEstablished {
                peer_role: Some(Role::PubSub),
                path: None
            })
        );
        Ok(())
    }

    #[test]
    fn client_sends_subscribe_after_session_established() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 21,
            data: {
                let mut bytes = BytesMut::new();
                let _ = MessageFramer::serialize_control_message(
                    ControlMessage::ServerSetup(ServerSetup {
                        supported_version: Version::Draft04,
                        role: Some(Role::PubSub),
                    }),
                    &mut bytes,
                )?;
                bytes.freeze()
            },
            fin: true,
        })?;
        let _ = protocol.poll_event();

        protocol.handle_write(Command::Subscribe {
            track_namespace: "foo".to_string(),
            track_name: "bar".to_string(),
            filter_type: FilterType::AbsoluteStart(FullSequence::new(4, 1)),
            authorization_info: Some("token".to_string()),
        })?;

        let Some(WriteOutput::SendStream {
            stream_id, bytes, ..
        }) = protocol.poll_write()
        else {
            panic!("expected subscribe bytes");
        };
        assert_eq!(stream_id, 21);

        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        match parser.poll_event() {
            Some(MessageParserEvent::ControlMessage(ControlMessage::Subscribe(subscribe))) => {
                assert_eq!(subscribe.subscribe_id, 0);
                assert_eq!(subscribe.track_alias, 0);
                assert_eq!(subscribe.track_namespace, "foo");
                assert_eq!(subscribe.track_name, "bar");
                assert_eq!(
                    subscribe.filter_type,
                    FilterType::AbsoluteStart(FullSequence::new(4, 1))
                );
                assert_eq!(subscribe.authorization_info, Some("token".to_string()));
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn client_receives_subscribe_ok_for_active_subscribe() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 23,
            data: {
                let mut bytes = BytesMut::new();
                let _ = MessageFramer::serialize_control_message(
                    ControlMessage::ServerSetup(ServerSetup {
                        supported_version: Version::Draft04,
                        role: Some(Role::PubSub),
                    }),
                    &mut bytes,
                )?;
                bytes.freeze()
            },
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::Subscribe {
            track_namespace: "foo".to_string(),
            track_name: "bar".to_string(),
            filter_type: FilterType::LatestObject,
            authorization_info: None,
        })?;
        let _ = protocol.poll_write();

        let mut subscribe_ok_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeOk(SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: Some(FullSequence::new(7, 2)),
            }),
            &mut subscribe_ok_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 23,
            data: subscribe_ok_bytes.freeze(),
            fin: true,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SubscribeAccepted {
                subscribe_id: 0,
                full_track_name: FullTrackName::new("foo".to_string(), "bar".to_string()),
                track_alias: 0,
                expires: 30,
                largest_group_object: Some(FullSequence::new(7, 2)),
            })
        );
        Ok(())
    }

    #[test]
    fn server_receives_subscribe_and_emits_event() -> Result<()> {
        let mut protocol = SessionCore::new(server_config(false));
        let mut client_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            }),
            &mut client_setup_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 25,
            data: client_setup_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_write();
        let _ = protocol.poll_event();

        let subscribe = Subscribe {
            subscribe_id: 7,
            track_alias: 9,
            track_namespace: "live".to_string(),
            track_name: "camera".to_string(),
            filter_type: FilterType::LatestGroup,
            authorization_info: None,
        };
        let mut subscribe_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(subscribe.clone()),
            &mut subscribe_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 25,
            data: subscribe_bytes.freeze(),
            fin: false,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SubscribeReceived(subscribe))
        );
        Ok(())
    }

    #[test]
    fn server_sends_subscribe_ok_command() -> Result<()> {
        let mut protocol = SessionCore::new(server_config(false));
        let mut client_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            }),
            &mut client_setup_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 27,
            data: client_setup_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_write();
        let _ = protocol.poll_event();
        let mut subscribe_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(Subscribe {
                subscribe_id: 3,
                track_alias: 4,
                track_namespace: "live".to_string(),
                track_name: "camera".to_string(),
                filter_type: FilterType::LatestGroup,
                authorization_info: None,
            }),
            &mut subscribe_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 27,
            data: subscribe_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();

        protocol.handle_write(Command::SubscribeOk {
            subscribe_id: 3,
            expires: 60,
            largest_group_object: None,
        })?;

        let Some(WriteOutput::SendStream { bytes, .. }) = protocol.poll_write() else {
            panic!("expected SUBSCRIBE_OK bytes");
        };
        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        match parser.poll_event() {
            Some(MessageParserEvent::ControlMessage(ControlMessage::SubscribeOk(subscribe_ok))) => {
                assert_eq!(subscribe_ok.subscribe_id, 3);
                assert_eq!(subscribe_ok.expires, 60);
                assert_eq!(subscribe_ok.largest_group_object, None);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn client_sends_subscribe_update_for_active_subscription() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 29,
            data: {
                let mut bytes = BytesMut::new();
                let _ = MessageFramer::serialize_control_message(
                    ControlMessage::ServerSetup(ServerSetup {
                        supported_version: Version::Draft04,
                        role: Some(Role::PubSub),
                    }),
                    &mut bytes,
                )?;
                bytes.freeze()
            },
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::Subscribe {
            track_namespace: "foo".to_string(),
            track_name: "bar".to_string(),
            filter_type: FilterType::LatestObject,
            authorization_info: None,
        })?;
        let _ = protocol.poll_write();

        let mut subscribe_ok_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeOk(SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: None,
            }),
            &mut subscribe_ok_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 29,
            data: subscribe_ok_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();

        protocol.handle_write(Command::SubscribeUpdate {
            subscribe_id: 0,
            start_group_object: FullSequence::new(8, 1),
            end_group_object: Some(FullSequence::new(10, 5)),
            authorization_info: None,
        })?;

        let Some(WriteOutput::SendStream { bytes, .. }) = protocol.poll_write() else {
            panic!("expected SUBSCRIBE_UPDATE bytes");
        };
        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        match parser.poll_event() {
            Some(MessageParserEvent::ControlMessage(ControlMessage::SubscribeUpdate(update))) => {
                assert_eq!(update.subscribe_id, 0);
                assert_eq!(update.start_group_object, FullSequence::new(8, 1));
                assert_eq!(update.end_group_object, Some(FullSequence::new(10, 5)));
                assert_eq!(update.authorization_info, None);
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn server_receives_subscribe_update_for_accepted_subscription() -> Result<()> {
        let mut protocol = SessionCore::new(server_config(false));
        let mut client_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            }),
            &mut client_setup_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 31,
            data: client_setup_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_write();
        let _ = protocol.poll_event();

        let subscribe = Subscribe {
            subscribe_id: 7,
            track_alias: 9,
            track_namespace: "live".to_string(),
            track_name: "camera".to_string(),
            filter_type: FilterType::LatestGroup,
            authorization_info: None,
        };
        let mut subscribe_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(subscribe),
            &mut subscribe_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 31,
            data: subscribe_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::SubscribeOk {
            subscribe_id: 7,
            expires: 60,
            largest_group_object: None,
        })?;
        let _ = protocol.poll_write();

        let update = SubscribeUpdate {
            subscribe_id: 7,
            start_group_object: FullSequence::new(3, 1),
            end_group_object: Some(FullSequence::new(5, 9)),
            authorization_info: Some("authz".to_string()),
        };
        let mut update_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeUpdate(update.clone()),
            &mut update_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 31,
            data: update_bytes.freeze(),
            fin: false,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SubscribeUpdated(update))
        );
        Ok(())
    }

    #[test]
    fn server_sends_subscribe_done_for_accepted_subscription() -> Result<()> {
        let mut protocol = SessionCore::new(server_config(false));
        let mut client_setup_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::ClientSetup(ClientSetup {
                supported_versions: vec![Version::Draft04],
                role: Some(Role::PubSub),
                path: Some("/moq".to_string()),
                uses_web_transport: false,
            }),
            &mut client_setup_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 33,
            data: client_setup_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_write();
        let _ = protocol.poll_event();

        let mut subscribe_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(Subscribe {
                subscribe_id: 7,
                track_alias: 9,
                track_namespace: "live".to_string(),
                track_name: "camera".to_string(),
                filter_type: FilterType::LatestGroup,
                authorization_info: None,
            }),
            &mut subscribe_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 33,
            data: subscribe_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::SubscribeOk {
            subscribe_id: 7,
            expires: 60,
            largest_group_object: None,
        })?;
        let _ = protocol.poll_write();

        protocol.handle_write(Command::SubscribeDone {
            subscribe_id: 7,
            status_code: 3,
            reason_phrase: "track ended".to_string(),
            final_group_object: Some(FullSequence::new(12, 4)),
        })?;

        let Some(WriteOutput::SendStream { bytes, .. }) = protocol.poll_write() else {
            panic!("expected SUBSCRIBE_DONE bytes");
        };
        let mut parser = MessageParser::new(false);
        parser.process_data(&mut bytes.as_ref(), false);
        match parser.poll_event() {
            Some(MessageParserEvent::ControlMessage(ControlMessage::SubscribeDone(done))) => {
                assert_eq!(done.subscribe_id, 7);
                assert_eq!(done.status_code, 3);
                assert_eq!(done.reason_phrase, "track ended");
                assert_eq!(done.final_group_object, Some(FullSequence::new(12, 4)));
            }
            _ => panic!("unexpected parser event"),
        }
        Ok(())
    }

    #[test]
    fn client_receives_subscribe_done_for_active_subscription() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 35,
            data: {
                let mut bytes = BytesMut::new();
                let _ = MessageFramer::serialize_control_message(
                    ControlMessage::ServerSetup(ServerSetup {
                        supported_version: Version::Draft04,
                        role: Some(Role::PubSub),
                    }),
                    &mut bytes,
                )?;
                bytes.freeze()
            },
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::Subscribe {
            track_namespace: "foo".to_string(),
            track_name: "bar".to_string(),
            filter_type: FilterType::LatestObject,
            authorization_info: None,
        })?;
        let _ = protocol.poll_write();

        let mut subscribe_ok_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeOk(SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: Some(FullSequence::new(7, 2)),
            }),
            &mut subscribe_ok_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 35,
            data: subscribe_ok_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();

        let mut subscribe_done_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeDone(SubscribeDone {
                subscribe_id: 0,
                status_code: 6,
                reason_phrase: "expired".to_string(),
                final_group_object: Some(FullSequence::new(9, 7)),
            }),
            &mut subscribe_done_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 35,
            data: subscribe_done_bytes.freeze(),
            fin: false,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::SubscribeEnded {
                subscribe_id: 0,
                full_track_name: FullTrackName::new("foo".to_string(), "bar".to_string()),
                track_alias: 0,
                status_code: 6,
                reason_phrase: "expired".to_string(),
                final_group_object: Some(FullSequence::new(9, 7)),
            })
        );
        Ok(())
    }

    #[test]
    fn client_receives_object_stream_for_active_subscription() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 41,
            data: {
                let mut bytes = BytesMut::new();
                let _ = MessageFramer::serialize_control_message(
                    ControlMessage::ServerSetup(ServerSetup {
                        supported_version: Version::Draft04,
                        role: Some(Role::PubSub),
                    }),
                    &mut bytes,
                )?;
                bytes.freeze()
            },
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::Subscribe {
            track_namespace: "foo".to_string(),
            track_name: "bar".to_string(),
            filter_type: FilterType::LatestObject,
            authorization_info: None,
        })?;
        let _ = protocol.poll_write();
        let mut subscribe_ok_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeOk(SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: None,
            }),
            &mut subscribe_ok_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 41,
            data: subscribe_ok_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();

        let object_header = ObjectHeader {
            subscribe_id: 0,
            track_alias: 0,
            group_id: 7,
            object_id: 2,
            object_send_order: 0,
            object_status: ObjectStatus::Normal,
            object_forwarding_preference: ObjectForwardingPreference::Object,
            object_payload_length: None,
        };
        let mut object_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_object(
            object_header,
            true,
            Bytes::from_static(b"abc"),
            &mut object_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 43,
            data: object_bytes.freeze(),
            fin: true,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::ObjectReceived {
                full_track_name: FullTrackName::new("foo".to_string(), "bar".to_string()),
                fragment: RemoteTrackOnObjectFragment {
                    object_header,
                    payload: Bytes::from_static(b"abc"),
                    fin: true,
                },
            })
        );
        Ok(())
    }

    #[test]
    fn client_buffers_partial_object_until_complete_when_disabled() -> Result<()> {
        let mut config = client_config(false);
        config.deliver_partial_objects = false;
        let mut protocol = SessionCore::new(config);
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 45,
            data: {
                let mut bytes = BytesMut::new();
                let _ = MessageFramer::serialize_control_message(
                    ControlMessage::ServerSetup(ServerSetup {
                        supported_version: Version::Draft04,
                        role: Some(Role::PubSub),
                    }),
                    &mut bytes,
                )?;
                bytes.freeze()
            },
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::Subscribe {
            track_namespace: "foo".to_string(),
            track_name: "bar".to_string(),
            filter_type: FilterType::LatestObject,
            authorization_info: None,
        })?;
        let _ = protocol.poll_write();
        let mut subscribe_ok_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeOk(SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: None,
            }),
            &mut subscribe_ok_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 45,
            data: subscribe_ok_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();

        let object_header = ObjectHeader {
            subscribe_id: 0,
            track_alias: 0,
            group_id: 1,
            object_id: 9,
            object_send_order: 0,
            object_status: ObjectStatus::Normal,
            object_forwarding_preference: ObjectForwardingPreference::Track,
            object_payload_length: Some(5),
        };
        let mut object_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_object(
            object_header,
            true,
            Bytes::from_static(b"hello"),
            &mut object_bytes,
        )?;
        let first = object_bytes.split_to(10).freeze();
        let second = object_bytes.freeze();

        protocol.handle_read(ReadInput::StreamData {
            stream_id: 47,
            data: first,
            fin: false,
        })?;
        assert_eq!(protocol.poll_event(), None);
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 47,
            data: second,
            fin: true,
        })?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::ObjectReceived {
                full_track_name: FullTrackName::new("foo".to_string(), "bar".to_string()),
                fragment: RemoteTrackOnObjectFragment {
                    object_header,
                    payload: Bytes::from_static(b"hello"),
                    fin: true,
                },
            })
        );
        Ok(())
    }

    #[test]
    fn client_receives_object_datagram_for_active_subscription() -> Result<()> {
        let mut protocol = SessionCore::new(client_config(false));
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 49,
            data: {
                let mut bytes = BytesMut::new();
                let _ = MessageFramer::serialize_control_message(
                    ControlMessage::ServerSetup(ServerSetup {
                        supported_version: Version::Draft04,
                        role: Some(Role::PubSub),
                    }),
                    &mut bytes,
                )?;
                bytes.freeze()
            },
            fin: false,
        })?;
        let _ = protocol.poll_event();
        protocol.handle_write(Command::Subscribe {
            track_namespace: "foo".to_string(),
            track_name: "bar".to_string(),
            filter_type: FilterType::LatestObject,
            authorization_info: None,
        })?;
        let _ = protocol.poll_write();
        let mut subscribe_ok_bytes = BytesMut::new();
        let _ = MessageFramer::serialize_control_message(
            ControlMessage::SubscribeOk(SubscribeOk {
                subscribe_id: 0,
                expires: 30,
                largest_group_object: None,
            }),
            &mut subscribe_ok_bytes,
        )?;
        protocol.handle_read(ReadInput::StreamData {
            stream_id: 49,
            data: subscribe_ok_bytes.freeze(),
            fin: false,
        })?;
        let _ = protocol.poll_event();

        let object_header = ObjectHeader {
            subscribe_id: 0,
            track_alias: 0,
            group_id: 3,
            object_id: 4,
            object_send_order: 0,
            object_status: ObjectStatus::Normal,
            object_forwarding_preference: ObjectForwardingPreference::Datagram,
            object_payload_length: None,
        };
        let mut datagram = BytesMut::new();
        let _ = MessageFramer::serialize_object_datagram(
            object_header,
            Bytes::from_static(b"xyz"),
            &mut datagram,
        )?;
        protocol.handle_read(ReadInput::Datagram(datagram.freeze()))?;

        assert_eq!(
            protocol.poll_event(),
            Some(EventOut::ObjectReceived {
                full_track_name: FullTrackName::new("foo".to_string(), "bar".to_string()),
                fragment: RemoteTrackOnObjectFragment {
                    object_header,
                    payload: Bytes::from_static(b"xyz"),
                    fin: true,
                },
            })
        );
        Ok(())
    }
}
