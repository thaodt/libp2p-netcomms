//! Protocol notification types for substream events.
//!
//! Contains types for protocol-specific notifications and events that occur
//! during substream operations, particularly for handling inbound streams.

use libp2p::{PeerId, StreamProtocol};

/// Event emitted when a new inbound substream is requested by a remote node.
#[derive(Debug, Clone)]
pub enum ProtocolEvent<TSubstream> {
    NewInboundSubstream {
        peer_id: PeerId,
        substream: TSubstream,
    },
}

/// Notification containing protocol-specific events
#[derive(Debug, Clone)]
pub struct ProtocolNotification<TSubstream> {
    /// The event that occurred
    pub event: ProtocolEvent<TSubstream>,
    /// The protocol associated with this notification
    pub protocol: StreamProtocol,
}

impl<TSubstream> ProtocolNotification<TSubstream> {
    pub fn new(protocol: StreamProtocol, event: ProtocolEvent<TSubstream>) -> Self {
        Self { event, protocol }
    }
}
