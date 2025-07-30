//! NØNOS IPC Message Types
//!
//! Provides structured and production-grade IPC message framing for ZeroState inter-module
//! communication. Encapsulates typed envelopes, headers, priority flags, delivery context,
//! and dispatchable payload categories.

use core::time::{Duration, SystemTime};
use alloc::vec::Vec;

/// Enum of all recognized IPC message categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    User,         // General module-level data
    System,       // Kernel/platform control or broadcast
    Signal,       // Control flow: ping, shutdown, suspend
    Capability,   // Capability tokens, handshakes, or revocation
    Error,        // Structured kernel or runtime errors
    Debug,        // Trace or telemetry messaging
    Auth,         // Authentication negotiation or requests
    Reserved(u8), // Reserved for future extensions
}

/// Bitflags for message header
pub mod MsgFlags {
    pub const PRIORITY_HIGH: u8 = 0b0000_0001;
    pub const ACK_REQUIRED: u8  = 0b0000_0010;
    pub const ENCRYPTED: u8     = 0b0000_0100;
    pub const SYSTEM_ONLY: u8   = 0b1000_0000;
}

/// Structured IPC message header for routing and introspection
#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub msg_type: MessageType,
    pub timestamp: Duration,
    pub flags: u8,
    pub sequence: u64,
    pub ttl: u8, // Optional future use: hop-count / routing TTL
}

/// Full IPC message envelope
#[derive(Debug, Clone)]
pub struct IpcEnvelope {
    pub header: MessageHeader,
    pub from: &'static str,
    pub to: &'static str,
    pub session_id: Option<&'static str>,
    pub data: Vec<u8>,
}

impl IpcEnvelope {
    /// Construct a typed message envelope
    pub fn new(
        msg_type: MessageType,
        from: &'static str,
        to: &'static str,
        data: &[u8],
        sequence: u64,
        flags: u8,
        ttl: u8,
        session_id: Option<&'static str>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0));

        Self {
            header: MessageHeader {
                msg_type,
                timestamp,
                flags,
                sequence,
                ttl,
            },
            from,
            to,
            session_id,
            data: data.to_vec(),
        }
    }

    pub fn is_control(&self) -> bool {
        matches!(
            self.header.msg_type,
            MessageType::System | MessageType::Signal | MessageType::Error
        )
    }

    pub fn is_user(&self) -> bool {
        self.header.msg_type == MessageType::User
    }

    pub fn is_encrypted(&self) -> bool {
        self.header.flags & MsgFlags::ENCRYPTED != 0
    }

    pub fn requires_ack(&self) -> bool {
        self.header.flags & MsgFlags::ACK_REQUIRED != 0
    }

    pub fn priority(&self) -> bool {
        self.header.flags & MsgFlags::PRIORITY_HIGH != 0
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

