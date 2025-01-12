//! Core types for substream management.
//!
//! Provides fundamental types and structures used throughout the substream system,
//! including stream identifiers, requests, and negotiated substream containers.

use std::fmt;

use libp2p::{PeerId, StreamProtocol};

/// type alias for stream id
pub type StreamId = u64;

/// Events that can be sent from the behavior to the handler
#[derive(Debug)]
pub enum FromBehaviorEvent {
    OpenRpcSessionRequest(OpenStreamRequest),
    AddSupportedProtocol(StreamProtocol),
}

impl From<OpenStreamRequest> for FromBehaviorEvent {
    fn from(event: OpenStreamRequest) -> Self {
        Self::OpenRpcSessionRequest(event)
    }
}

#[derive(Debug)]
pub struct OpenStreamRequest {
    stream_id: StreamId,
    peer_id: PeerId,
    protocol: StreamProtocol,
}

impl OpenStreamRequest {
    pub fn new(stream_id: StreamId, peer_id: PeerId, protocol: StreamProtocol) -> Self {
        Self {
            stream_id,
            peer_id,
            protocol,
        }
    }

    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    pub fn protocol(&self) -> &StreamProtocol {
        &self.protocol
    }
}

/// Contains the substream and the ProtocolId that was successfully negotiated.
pub struct NegotiatedSubstream<TSubstream> {
    pub peer_id: PeerId,
    pub protocol: StreamProtocol,
    pub stream: TSubstream,
}

impl<TSubstream> NegotiatedSubstream<TSubstream> {
    pub fn new(peer_id: PeerId, protocol: StreamProtocol, stream: TSubstream) -> Self {
        Self {
            peer_id,
            protocol,
            stream,
        }
    }
}

/// impl Debug for NegotiatedSubstream
impl<TSubstream> fmt::Debug for NegotiatedSubstream<TSubstream> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NegotiatedSubstream")
            .field("peer_id", &format!("{:?}", self.peer_id))
            .field("protocol", &format!("{:?}", self.protocol))
            .field("stream", &"...".to_string())
            .finish()
    }
}
