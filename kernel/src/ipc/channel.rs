//! NØNOS Inter-Process Communication (IPC) Subsystem
//!
//! Provides capability-enforced, memory-safe message channels between `.mod` instances.
//! This subsystem forms the internal microbus for ZeroState module communication. Channels
//! are enforced through declared IPC capabilities and designed for high-assurance sandboxing.

use crate::capabilities::{Capability, CapabilityToken};
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::{collections::VecDeque, string::String, sync::Arc};
use spin::Mutex;

/// Maximum payload size per IPC message (bytes)
pub const MAX_MSG_SIZE: usize = 256;
/// Maximum number of messages per channel queue
pub const MAX_QUEUE_DEPTH: usize = 64;
/// Maximum number of active IPC channels system-wide
pub const MAX_CHANNELS: usize = 32;

/// Represents a single message between modules.
#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub from: &'static str,
    pub to: &'static str,
    pub payload: [u8; MAX_MSG_SIZE],
    pub len: usize,
}

impl IpcMessage {
    pub fn new(from: &'static str, to: &'static str, data: &[u8]) -> Result<Self, &'static str> {
        if data.len() > MAX_MSG_SIZE {
            return Err("IPC message exceeds max length");
        }
        let mut payload = [0u8; MAX_MSG_SIZE];
        payload[..data.len()].copy_from_slice(data);
        Ok(Self {
            from,
            to,
            payload,
            len: data.len(),
        })
    }
}

/// Internal channel structure with synchronized message queue.
#[derive(Debug)]
pub struct IpcChannel {
    pub from: &'static str,
    pub to: &'static str,
    pub queue: Mutex<VecDeque<IpcMessage>>,
    pub access_token: CapabilityToken,
}

impl IpcChannel {
    pub fn new(from: &'static str, to: &'static str, token: CapabilityToken) -> Self {
        Self {
            from,
            to,
            queue: Mutex::new(VecDeque::with_capacity(MAX_QUEUE_DEPTH)),
            access_token: token,
        }
    }

    /// Send a message to the channel queue.
    pub fn send(&self, msg: IpcMessage) -> Result<(), &'static str> {
        if msg.len > MAX_MSG_SIZE {
            return Err("IPC message too large");
        }
        let mut queue = self.queue.lock();
        if queue.len() >= MAX_QUEUE_DEPTH {
            return Err("IPC queue full");
        }
        queue.push_back(msg);
        Ok(())
    }

    /// Receive a message from the channel queue.
    pub fn receive(&self) -> Option<IpcMessage> {
        self.queue.lock().pop_front()
    }

    /// Peek the next message without removing it.
    pub fn peek(&self) -> Option<IpcMessage> {
        self.queue.lock().front().cloned()
    }
}

/// Global IPC bus managing multiple channels.
#[derive(Debug)]
pub struct IpcBus {
    pub channels: Mutex<[Option<Arc<IpcChannel>>; MAX_CHANNELS]>,
    pub active_count: AtomicUsize,
}

impl IpcBus {
    pub const fn new() -> Self {
        const NONE: Option<Arc<IpcChannel>> = None;
        Self {
            channels: Mutex::new([NONE; MAX_CHANNELS]),
            active_count: AtomicUsize::new(0),
        }
    }

    /// Open a new channel between modules with access verification.
    pub fn open_channel(
        &self,
        from: &'static str,
        to: &'static str,
        token: CapabilityToken,
    ) -> Result<(), &'static str> {
        if !token.permissions.contains(&Capability::IPC) {
            return Err("Permission denied: module lacks IPC capability");
        }

        let mut slots = self.channels.lock();
        for slot in slots.iter_mut() {
            if slot.is_none() {
                let channel = Arc::new(IpcChannel::new(from, to, token));
                *slot = Some(channel);
                self.active_count.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
        }
        Err("Maximum IPC channels reached")
    }

    /// Find an active channel by source and destination.
    pub fn find_channel(&self, from: &str, to: &str) -> Option<Arc<IpcChannel>> {
        let slots = self.channels.lock();
        for slot in slots.iter() {
            if let Some(ref ch) = slot {
                if ch.from == from && ch.to == to {
                    return Some(ch.clone());
                }
            }
        }
        None
    }

    /// List all active channel routes.
    pub fn list_routes(&self) -> Vec<(String, String)> {
        let slots = self.channels.lock();
        slots
            .iter()
            .filter_map(|slot| slot.as_ref())
            .map(|ch| (ch.from.to_string(), ch.to.to_string()))
            .collect()
    }
}

/// Global singleton IPC bus instance
pub static IPC_BUS: IpcBus = IpcBus::new();
