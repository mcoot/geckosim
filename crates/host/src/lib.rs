//! Gecko-sim host library: the WebSocket transport surface.
//!
//! The `gecko-sim-host` binary (`src/main.rs`) is a thin wrapper that
//! constructs a `Sim`, channels, and a `TcpListener`, then spawns the
//! `sim_driver` and `ws_server` tasks defined here. Exposing them as a
//! library lets `tests/ws_smoke.rs` drive the same code paths in-process
//! against an ephemeral listener.

pub mod config;
pub mod sim_driver;
pub mod ws_server;
