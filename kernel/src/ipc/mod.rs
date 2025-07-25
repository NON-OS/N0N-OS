//! NØNOS IPC Subsystem Entrypoint
//!
//! Unified IPC module re-exports the entire inter-process communication system.
//! It serves as the primary messaging backbone of ZeroState, offering secure,
//! capability-scoped, session-aware, and optionally encrypted communication between
//! isolated `.mod` sandboxed environments.

#![allow(unused_imports)]

pub mod channel;
pub mod message;
pub mod policy;
pub mod transport;

use crate::capabilities::CapabilityToken;
use channel::{IPC_BUS, IpcChannel, IpcMessage};
use message::{IpcEnvelope, MessageType};
use policy::{IpcPolicy, ACTIVE_POLICY};
use transport::{IpcStream, send_stream_payload};
use alloc::vec::Vec;
use log::{info, warn};

/// IPC System Diagnostic Report
#[derive(Debug, Clone)]
pub struct IpcStatus {
    pub active_routes: usize,
    pub open_streams: usize,
    pub messages_in_flight: usize,
}

/// Initialize the IPC subsystem and prepare bus
pub fn init_ipc() {
    info!(target: "ipc", "Initializing NØNOS IPC bus...");
    // TODO: register runtime signals, IPC watchdog, etc.
    info!(target: "ipc", "IPC subsystem active.");
}

/// Attempt to send a validated IPC envelope
pub fn send_envelope(
    envelope: IpcEnvelope,
    token: &CapabilityToken,
) -> Result<(), &'static str> {
    unsafe {
        if !ACTIVE_POLICY.allow_message(&envelope, token) {
            warn!(target: "ipc::policy", "Message rejected by policy: {:?}", envelope);
            return Err("IPC policy violation: send denied");
        }
    }

    if let Some(channel) = IPC_BUS.find_channel(envelope.from, envelope.to) {
        channel.send(IpcMessage::new(envelope.from, envelope.to, &envelope.data)?)
    } else {
        Err("No IPC channel found")
    }
}

/// Send a large payload via transport framing
pub fn send_stream(
    stream: &IpcStream,
    payload: &[u8],
    token: &CapabilityToken,
) -> Result<(), &'static str> {
    let tx = |env: IpcEnvelope| send_envelope(env, token);
    send_stream_payload(stream, payload, tx)
}

/// List all active module-to-module IPC routes
pub fn list_routes() -> Vec<(String, String)> {
    IPC_BUS.list_routes()
}

/// Retrieve real-time diagnostic report of IPC state
pub fn get_ipc_status() -> IpcStatus {
    IpcStatus {
        active_routes: IPC_BUS.list_routes().len(),
        open_streams: 0, // Placeholder: later track open IpcStream instances
        messages_in_flight: 0, // Hook into scheduler or channel telemetry
    }
}

/// Register a new IPC channel if permitted
pub fn open_secure_channel(
    from: &'static str,
    to: &'static str,
    token: &CapabilityToken,
) -> Result<(), &'static str> {
    unsafe {
        if !ACTIVE_POLICY.allow_channel(from, to, token) {
            return Err("IPC policy violation: open_channel denied");
        }
    }

    IPC_BUS.open_channel(from, to)
}
