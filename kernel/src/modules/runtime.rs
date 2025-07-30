//! NØNOS Module Runtime Capsule
//!
//! This subsystem defines the fully sandboxed, per-module runtime capsule.
//! Capsules are embedded in sandbox contexts and never globally visible.
//!
//! Responsibilities:
//! - Execution metrics (uptime, restarts, IPC, memory)
//! - Lifecycle enforcement (fault, suspend, teardown)
//! - Capability-authenticated introspection
//! - RAM-only; cleared on ZeroState destruction

use crate::capabilities::{Capability, CapabilityToken};
use crate::modules::sandbox::FaultPolicy;
use crate::log::logger::{log_warn, log_info};
use crate::syscall::mod_interface::ModSyscall;
use crate::crypto::zk::generate_session_entropy;

use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use alloc::string::String;
use alloc::collections::BTreeMap;

/// Globally unique session counter (non-persistent)
static SESSION_SEQ: AtomicU64 = AtomicU64::new(1000);

/// Runtime telemetry snapshot
#[derive(Debug, Clone, Default)]
pub struct RuntimeMetrics {
    pub restarts: u32,
    pub faults: u32,
    pub ipc_sent: u64,
    pub ipc_recv: u64,
    pub boot_time_ticks: u64,
    pub memory_bytes: usize,
    pub entropy_seed: [u8; 32],
}

/// Runtime control capsule for an executing `.mod` binary
#[derive(Debug)]
pub struct RuntimeCapsule {
    pub session_id: u64,
    pub module_name: &'static str,
    pub token: CapabilityToken,
    pub policy: FaultPolicy,
    pub metrics: RuntimeMetrics,
    pub active: bool,
    pub annotations: BTreeMap<&'static str, &'static str>,
}

impl RuntimeCapsule {
    pub fn new(module: &'static str, token: CapabilityToken, policy: FaultPolicy, memory_bytes: usize) -> Self {
        Self {
            session_id: SESSION_SEQ.fetch_add(1, Ordering::Relaxed),
            module_name: module,
            token,
            policy,
            metrics: RuntimeMetrics {
                memory_bytes,
                entropy_seed: generate_session_entropy(),
                ..Default::default()
            },
            active: true,
            annotations: BTreeMap::new(),
        }
    }

    /// Handle a fatal fault by policy
    pub fn apply_fault(&mut self) {
        self.metrics.faults += 1;

        match self.policy {
            FaultPolicy::Terminate => {
                self.active = false;
                log_warn("runtime", &format!("Module '{}' terminated on fault", self.module_name));
            }
            FaultPolicy::Restart => {
                self.metrics.restarts += 1;
                log_warn("runtime", &format!("Module '{}' faulted — restart queued", self.module_name));
                // actual restart is external (mod_runner)
            }
            FaultPolicy::Isolate => {
                self.active = false;
                log_warn("runtime", &format!("Module '{}' isolated from scheduler", self.module_name));
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn mark_inactive(&mut self) {
        self.active = false;
    }

    /// Update IPC metrics
    pub fn record_ipc(&mut self, direction: IPCDirection) {
        match direction {
            IPCDirection::Sent => self.metrics.ipc_sent += 1,
            IPCDirection::Received => self.metrics.ipc_recv += 1,
        }
    }

    /// Annotate runtime state with debug label
    pub fn annotate(&mut self, key: &'static str, value: &'static str) {
        self.annotations.insert(key, value);
    }

    /// Kernel-syscall readable summary
    pub fn describe(&self, caller: &CapabilityToken) -> Option<RuntimeCapsuleInfo> {
        if !caller.allows(Capability::Debug) {
            return None;
        }

        Some(RuntimeCapsuleInfo {
            module: self.module_name,
            session_id: self.session_id,
            memory: self.metrics.memory_bytes,
            restarts: self.metrics.restarts,
            faults: self.metrics.faults,
            active: self.active,
            boot_ticks: self.metrics.boot_time_ticks,
            annotations: self.annotations.clone(),
        })
    }
}

/// Lightweight debug representation exposed via syscall with Capability::Debug
#[derive(Debug, Clone)]
pub struct RuntimeCapsuleInfo {
    pub module: &'static str,
    pub session_id: u64,
    pub memory: usize,
    pub restarts: u32,
    pub faults: u32,
    pub active: bool,
    pub boot_ticks: u64,
    pub annotations: BTreeMap<&'static str, &'static str>,
}

/// IPC operation direction enum
#[derive(Debug, Copy, Clone)]
pub enum IPCDirection {
    Sent,
    Received,
}
