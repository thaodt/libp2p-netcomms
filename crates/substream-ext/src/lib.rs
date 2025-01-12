//! Substream handling for libp2p connections
//!
//! This module provides functionality for managing substreams in a libp2p network,
//! including stream creation, protocol negotiation and event handling.

mod behavior;
pub mod error;
mod event;
mod handler;
mod notifier;
mod substream_types;

pub use behavior::*;
pub use error::Error;
pub use event::*;
pub use notifier::*;
pub use substream_types::*;
