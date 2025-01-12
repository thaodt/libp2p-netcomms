//! Error types for the substream extension module.
//!
//! This module defines the error types that can occur during substream operations,
//! including connection, protocol negotiation, and communication failures.
use std::fmt::{Debug, Display, Formatter};

/// Represents errors that can occur during substream operations
#[derive(Debug, Clone)]
pub enum Error {
    /// Indicates that an established connection was terminated
    ConnectionClosed,
    /// Represents a failure to establish a connection with details about why
    DialFailure { details: String },
    /// No known addresses available for the peer
    NoAddressesForPeer,
    /// Represents a failure during the protocol upgrade process after dialing
    DialUpgradeError,
    /// the remote peer doesn't support our requested protocol
    ProtocolNotSupported,
    /// the protocol negotiation process timed out
    ProtocolNegotiationTimeout,
    /// an internal channel used for communication was closed
    ChannelClosed,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionClosed => write!(f, "Connection closed"),
            Self::DialFailure { details } => write!(f, "Dial failure: {details}"),
            Self::NoAddressesForPeer => write!(f, "No addresses for peer"),
            Self::DialUpgradeError => write!(f, "Dial upgrade error"),
            Self::ProtocolNotSupported => write!(f, "Protocol not supported"),
            Self::ProtocolNegotiationTimeout => write!(f, "Protocol negotiation timeout"),
            Self::ChannelClosed => write!(f, "Channel closed"),
        }
    }
}

impl std::error::Error for Error {}
