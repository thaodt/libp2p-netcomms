//! Network behavior implementation for substream management.
//!
//! This module provides the core behavior implementation for managing substreams in a libp2p network.
//! It handles:
//! - Connection establishment and teardown
//! - Protocol negotiation
//! - Stream management
//! - Event routing between the swarm and connection handlers

use std::{
    collections::{HashMap, HashSet, VecDeque},
    task::{Context, Poll},
};

use libp2p::{
    core::{transport::PortUse, Endpoint},
    swarm::{
        dial_opts::DialOpts, AddressChange, ConnectionClosed, ConnectionDenied, ConnectionHandler,
        ConnectionId, DialError, DialFailure, FromSwarm, NetworkBehaviour, NotifyHandler, THandler,
        THandlerInEvent, THandlerOutEvent, ToSwarm,
    },
    Multiaddr, PeerId, StreamProtocol,
};
use smallvec::SmallVec;

use crate::{
    error::Error, event::Event, handler::Handler, substream_types::StreamId, FromBehaviorEvent,
    OpenStreamRequest,
};

/// Maximum capacity threshold for empty queues before memory reclamation.
/// When an empty queue's capacity exceeds this value, its underlying memory
/// will be released to optimize resource usage.
pub const MAX_IDLE_QUEUE_SIZE: usize = 100;

/// Network behavior implementation for managing substream connections in libp2p.
/// Handles connection establishment, protocol negotiation, and stream management.
#[derive(Debug)]
pub struct Behavior {
    /// Supported protocols
    protocols: SmallVec<[StreamProtocol; 32]>,
    /// Queue of events to be processed by the swarm
    pending_events: VecDeque<ToSwarm<Event, THandlerInEvent<Self>>>,
    /// the connected peers and their connection states
    connected: HashMap<PeerId, SmallVec<[Connection; 2]>>,
    /// Pending outbound stream requests for peers we're not yet connected to
    pending_outbound_streams: HashMap<PeerId, SmallVec<[OpenStreamRequest; 10]>>,
    /// Counter for generating unique stream IDs
    next_outbound_stream_id: StreamId,
}

impl Behavior {
    pub fn new<I: IntoIterator<Item = StreamProtocol>>(protocols: I) -> Self {
        Self {
            protocols: protocols.into_iter().collect(),
            pending_events: VecDeque::new(),
            pending_outbound_streams: HashMap::new(),
            connected: HashMap::new(),
            next_outbound_stream_id: StreamId::default(),
        }
    }

    /// Adds a new protocol to the behavior and notifies all active connections
    pub fn add_protocol(&mut self, protocol: StreamProtocol) {
        self.protocols.push(protocol.clone());
        // Notify all active connections
        for (peer_id, connections) in &self.connected {
            for conn in connections {
                self.pending_events.push_back(ToSwarm::NotifyHandler {
                    peer_id: *peer_id,
                    handler: NotifyHandler::One(conn.id),
                    event: FromBehaviorEvent::AddSupportedProtocol(protocol.clone()),
                });
            }
        }
    }

    /// returns the supported protocols [getter]
    pub fn supported_protocols(&self) -> &[StreamProtocol] {
        &self.protocols
    }

    /// Opens a new substream to the specified peer using the given protocol
    /// Returns a unique StreamId for the new stream
    pub fn open_substream(&mut self, peer_id: PeerId, protocol: StreamProtocol) -> StreamId {
        let stream_id = self.next_outbound_stream_id();
        let request = OpenStreamRequest::new(stream_id, peer_id, protocol);

        match self.get_connections(&peer_id) {
            Some(connections) => {
                let ix = (stream_id as usize) % connections.len();
                let conn = &mut connections[ix];
                conn.pending_streams.insert(stream_id);
                let conn_id = conn.id;
                self.pending_events.push_back(ToSwarm::NotifyHandler {
                    peer_id,
                    handler: NotifyHandler::One(conn_id),
                    event: request.into(),
                });
            }
            None => {
                self.pending_events.push_back(ToSwarm::Dial {
                    opts: DialOpts::peer_id(peer_id).build(),
                });
                self.pending_outbound_streams
                    .entry(peer_id)
                    .or_default()
                    .push(request);
            }
        }
        stream_id
    }

    /// Returns the next available stream id for a new outbound stream
    fn next_outbound_stream_id(&mut self) -> StreamId {
        let request_id = self.next_outbound_stream_id;
        self.next_outbound_stream_id = self.next_outbound_stream_id.wrapping_add(1);
        request_id
    }

    /// Handles a closed connection event to a peer.
    fn on_conn_closed(
        &mut self,
        ConnectionClosed {
            peer_id,
            connection_id,
            remaining_established,
            ..
        }: ConnectionClosed,
    ) {
        let connections = self
            .connected
            .get_mut(&peer_id)
            .expect("Expected some established connection to peer before closing.");

        let connection = connections
            .iter()
            .position(|c| c.id == connection_id)
            .map(|p: usize| connections.remove(p))
            .expect("Expected connection to be established before closing.");

        debug_assert_eq!(connections.is_empty(), remaining_established == 0);
        if connections.is_empty() {
            self.connected.remove(&peer_id);
        }

        for stream_id in connection.pending_streams {
            self.pending_events
                .push_back(ToSwarm::GenerateEvent(Event::InboundFailure {
                    peer_id,
                    stream_id,
                    error: Error::ConnectionClosed,
                }));
        }
    }

    /// Handles the [`libp2p_core::connection::ConnectedPoint``] of an existing connection has changed.
    fn on_address_change(&mut self, address_change: AddressChange) {
        let AddressChange {
            peer_id,
            connection_id,
            new,
            ..
        } = address_change;
        if let Some(connections) = self.connected.get_mut(&peer_id) {
            for connection in connections {
                if connection.id == connection_id {
                    connection.remote_address = Some(new.get_remote_address().clone());
                    return;
                }
            }
        }
    }

    /// Handles a peer connection failure.
    fn handle_failed_peer_connection(&mut self, DialFailure { peer_id, error, .. }: DialFailure) {
        if matches!(error, DialError::DialPeerConditionFalse(_)) {
            return;
        }

        if let Some(peer) = peer_id {
            // If there are pending outgoing stream requests when a dial failure occurs,
            // it is implied that we are not connected to the peer, since pending
            // outgoing stream requests are drained when a connection is established and
            // only created when a peer is not connected when a request is made.
            // Therefore these requests must be considered failed, even if there is
            // another, concurrent dialing attempt ongoing.
            if let Some(pending) = self.pending_outbound_streams.remove(&peer) {
                let no_addresses = matches!(&error, DialError::NoAddresses);
                for request in pending {
                    self.pending_events
                        .push_back(ToSwarm::GenerateEvent(Event::OutboundFailure {
                            peer_id: peer,
                            protocol: request.protocol().clone(),
                            stream_id: request.stream_id(),
                            error: if no_addresses {
                                Error::NoAddressesForPeer
                            } else {
                                Error::DialFailure {
                                    details: error.to_string(),
                                }
                            },
                        }));
                }
            }
        }
    }

    /// Handles a new connection being established.
    fn on_conn_established(
        &mut self,
        handler: &mut Handler,
        peer_id: PeerId,
        connection_id: ConnectionId,
        remote_address: Option<Multiaddr>,
    ) {
        let mut connection = Connection::new(connection_id, remote_address);

        if let Some(pending_streams) = self.pending_outbound_streams.remove(&peer_id) {
            for stream in pending_streams {
                connection.pending_streams.insert(stream.stream_id());
                handler.on_behaviour_event(stream.into());
            }
        }

        self.connected.entry(peer_id).or_default().push(connection);
    }

    fn get_connections(&mut self, peer_id: &PeerId) -> Option<&mut SmallVec<[Connection; 2]>> {
        self.connected.get_mut(peer_id).filter(|c| !c.is_empty())
    }
}

impl NetworkBehaviour for Behavior {
    type ConnectionHandler = Handler;
    type ToSwarm = Event;

    /// Handles a new inbound connection that has been established
    /// Returns a new handler for the connection or denies it
    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        let mut handler = Handler::new(peer, self.protocols.clone());
        self.on_conn_established(&mut handler, peer, connection_id, Some(remote_addr.clone()));

        Ok(handler)
    }

    /// Handles a new outbound connection that has been established
    /// Returns a new handler for the connection or denies it
    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        remote_addr: &Multiaddr,
        _role_override: Endpoint,
        _port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        let mut handler = Handler::new(peer, self.protocols.clone());
        self.on_conn_established(&mut handler, peer, connection_id, Some(remote_addr.clone()));
        Ok(handler)
    }

    /// Processes events received from the swarm
    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(_) => {}
            FromSwarm::ConnectionClosed(connection_closed) => {
                self.on_conn_closed(connection_closed)
            }
            FromSwarm::AddressChange(address_change) => self.on_address_change(address_change),
            FromSwarm::DialFailure(dial_failure) => {
                self.handle_failed_peer_connection(dial_failure)
            }
            _ => {}
        }
    }

    /// Processes events received from the connection handler
    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match &event {
            Event::SubstreamOpen {
                peer_id, stream_id, ..
            } => {
                if let Some(connections) = self.connected.get_mut(peer_id) {
                    for connection in connections {
                        connection.pending_streams.remove(stream_id);
                    }
                }
            }
            Event::InboundSubstreamOpen { .. } => {}
            Event::InboundFailure { .. } => {}
            Event::OutboundFailure { .. } => {}
            Event::Error(_) => {}
        }

        self.pending_events.push_back(ToSwarm::GenerateEvent(event));
    }

    /// Polls the behavior for pending events to be sent to the swarm
    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(event);
        }
        if self.pending_events.capacity() > MAX_IDLE_QUEUE_SIZE {
            self.pending_events.shrink_to_fit();
        }

        Poll::Pending
    }
}

/// Tracking internal infor for an established connection with its associated state.
#[derive(Debug)]
struct Connection {
    id: ConnectionId,
    remote_address: Option<Multiaddr>,
    pending_streams: HashSet<StreamId>,
}

impl Connection {
    fn new(id: ConnectionId, remote_address: Option<Multiaddr>) -> Self {
        Self {
            id,
            remote_address,
            pending_streams: HashSet::new(),
        }
    }
}
