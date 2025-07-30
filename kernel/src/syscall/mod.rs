//! NØNOS Syscall Dispatch Module
//!
//! This module defines the secure system call dispatcher used by the microkernel.
//! It maps incoming syscall numbers to kernel services and verifies permissions
//! using capability tokens assigned to executing modules. Each call is guarded
//! with zero-trust policies defined in `capabilities.rs`.

pub mod capabilities;

use crate::syscall::capabilities::{Capability, verify_capability};
use crate::log::logger::try_get_logger;

/// System call operation codes
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u64)]
pub enum Syscall {
    Log = 0x01,
    GetTime = 0x02,
    SecureWrite = 0x03,
    ModSpawn = 0x04,
    ReadEntropy = 0x05,
    IPCSend = 0x06,
    IPCReceive = 0x07,
}

impl Syscall {
    pub fn from_raw(val: u64) -> Option<Self> {
        match val {
            0x01 => Some(Syscall::Log),
            0x02 => Some(Syscall::GetTime),
            0x03 => Some(Syscall::SecureWrite),
            0x04 => Some(Syscall::ModSpawn),
            0x05 => Some(Syscall::ReadEntropy),
            0x06 => Some(Syscall::IPCSend),
            0x07 => Some(Syscall::IPCReceive),
            _ => None,
        }
    }
}

/// Entry point from syscall stub (typically invoked via syscall instruction)
pub fn handle_syscall(syscall_id: u64, arg0: u64, arg1: u64) -> u64 {
    match Syscall::from_raw(syscall_id) {
        Some(Syscall::Log) => {
            enforce(Capability::IO, || {
                log("[SYSCALL] Log called");
                0
            })
        },
        Some(Syscall::GetTime) => {
            enforce(Capability::CoreExec, || {
                1689357890 // Stub Unix timestamp
            })
        },
        Some(Syscall::SecureWrite) => {
            enforce(Capability::SecureMem, || {
                log("[SYSCALL] Secure write allowed");
                1
            })
        },
        Some(Syscall::ModSpawn) => {
            enforce(Capability::CoreExec, || {
                log("[SYSCALL] Module spawn (stub)");
                42
            })
        },
        Some(Syscall::ReadEntropy) => {
            enforce(Capability::Crypto, || {
                0xA5A5A5A5 // Stub entropy
            })
        },
        Some(Syscall::IPCSend) => {
            enforce(Capability::IPC, || {
                log("[SYSCALL] IPC send");
                0
            })
        },
        Some(Syscall::IPCReceive) => {
            enforce(Capability::IPC, || {
                log("[SYSCALL] IPC receive");
                0
            })
        },
        None => {
            deny("Unknown syscall")
        },
    }
}

/// Enforces a capability before executing syscall body
fn enforce<F: FnOnce() -> u64>(required: Capability, op: F) -> u64 {
    if verify_capability(required) {
        op()
    } else {
        deny("Capability check failed")
    }
}

/// Logs and denies the request
fn deny(reason: &str) -> u64 {
    log(&format!("[SYSCALL] Denied: {}", reason));
    u64::MAX
}

/// Internal kernel log interface
fn log(msg: &str) {
    if let Some(log) = try_get_logger() {
        log.log(msg);
    }
}
