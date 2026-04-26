//! Gecko-sim protocol: wire types for the host ↔ frontend WebSocket channel.
//! See `messages` for the envelope enums and ADR 0013 for the design.

pub mod messages;

pub use messages::{ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION};
