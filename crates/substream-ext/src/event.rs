//! Event types for substream operations.
//!
//! Defines the events that can be emitted during substream lifecycle including
//! successful connections, failures, and protocol-specific events.

use libp2p::{PeerId, Stream, StreamProtocol};

use crate::{error::Error, substream_types::StreamId, ProtocolNotification};

/// Events that can be emitted by the substream
#[derive(Debug)]
pub enum Event {
    SubstreamOpen {
        peer_id: PeerId,
        stream_id: StreamId,
        stream: Stream,
        protocol: StreamProtocol,
    },
    InboundSubstreamOpen {
        notification: ProtocolNotification<Stream>,
    },
    InboundFailure {
        peer_id: PeerId,
        stream_id: StreamId,
        error: Error,
    },
    OutboundFailure {
        peer_id: PeerId,
        protocol: StreamProtocol,
        stream_id: StreamId,
        error: Error,
    },
    Error(Error),
}
