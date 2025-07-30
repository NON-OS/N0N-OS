//! NØNOS Capsule Runtime Lifecycle
//!
//! Handles full execution lifecycle of modules:
//! - Execution state transitions
//! - Fault detection and policy resolution
//! - Secure telemetry (heartbeat, attestation)
//! - zkSnapshot generation for cryptographic relay export
//! - Fully memory-aware and restart-compatible

use crate::capabilities::CapabilityToken;
use crate::crypto::zk::{AttestationProof, generate_snapshot_signature};
use crate::log::logger::{log_info, log_warn};

use core::time::{Duration, Instant};
use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsuleState {
    Inactive,
    Active,
    Suspended,
    Faulted,
    Terminating,
    Restarting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPolicy {
    /// Restart capsule once fault is detected (default)
    Restart,
    /// Gracefully shut down the capsule
    Shutdown,
    /// Escalate to system-wide trap
    Escalate,
    /// Ignore fault and suspend capsule
    Suspend,
}

#[derive(Debug, Clone)]
pub struct RuntimeCapsule {
    pub name: &'static str,
    pub token: CapabilityToken,
    pub policy: FaultPolicy,
    pub memory_bytes: usize,
    pub state: CapsuleState,
    last_heartbeat: Instant,
    launch_time: Instant,
}

impl RuntimeCapsule {
    /// Construct a new live runtime capsule instance
    pub fn new(name: &'static str, token: CapabilityToken, policy: FaultPolicy, memory_bytes: usize) -> Self {
        let now = Instant::now();
        log_info("runtime", &format!("Capsule '{}' created | policy: {:?} | mem={} KB", name, policy, memory_bytes / 1024));
        Self {
            name,
            token,
            policy,
            memory_bytes,
            state: CapsuleState::Active,
            last_heartbeat: now,
            launch_time: now,
        }
    }

    /// Return true if capsule is live
    pub fn is_active(&self) -> bool {
        matches!(self.state, CapsuleState::Active)
    }

    /// Lifecycle transition: mark capsule inactive
    pub fn mark_inactive(&mut self) {
        self.state = CapsuleState::Inactive;
        log_info("runtime", &format!("Capsule '{}' marked Inactive", self.name));
    }

    /// Lifecycle transition: suspend capsule
    pub fn suspend(&mut self) {
        self.state = CapsuleState::Suspended;
        log_warn("runtime", &format!("Capsule '{}' suspended due to soft fault", self.name));
    }

    /// Lifecycle transition: faulted
    pub fn fault(&mut self) {
        self.state = CapsuleState::Faulted;
        log_warn("runtime", &format!("Capsule '{}' entered Faulted state", self.name));
        self.resolve_policy();
    }

    /// Lifecycle transition: termination
    pub fn terminate(&mut self) {
        self.state = CapsuleState::Terminating;
        log_warn("runtime", &format!("Capsule '{}' is terminating", self.name));
    }

    /// Apply fault policy after failure
    fn resolve_policy(&mut self) {
        match self.policy {
            FaultPolicy::Restart => {
                self.state = CapsuleState::Restarting;
                log_info("runtime", &format!("Capsule '{}' set to restart", self.name));
            }
            FaultPolicy::Shutdown => {
                self.state = CapsuleState::Terminating;
                log_info("runtime", &format!("Capsule '{}' set to shutdown", self.name));
            }
            FaultPolicy::Suspend => self.suspend(),
            FaultPolicy::Escalate => {
                // TODO: Signal system-wide fault escalation mechanism
                log_warn("runtime", &format!("Capsule '{}' triggered escalation", self.name));
            }
        }
    }

    /// Update capsule heartbeat (activity tick)
    pub fn tick(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    /// Seconds since last activity tick
    pub fn last_seen(&self) -> Duration {
        self.last_heartbeat.elapsed()
    }

    /// Uptime since capsule launched
    pub fn uptime(&self) -> Duration {
        self.launch_time.elapsed()
    }

    /// Memory footprint in bytes
    pub fn memory_bytes(&self) -> usize {
        self.memory_bytes
    }

    /// Current runtime state
    pub fn state(&self) -> CapsuleState {
        self.state
    }

    /// Current fault handling policy
    pub fn fault_policy(&self) -> FaultPolicy {
        self.policy
    }

    /// Export cryptographic zkSnapshot (signed execution metadata)
    pub fn generate_signed_snapshot(&self, exec_id: [u8; 32]) -> [u8; 64] {
        generate_snapshot_signature(exec_id, &self.token, self.memory_bytes, self.state)
    }

    /// Export high-level attestation proof (for zkRelay export)
    pub fn attestation(&self, exec_id: [u8; 32]) -> AttestationProof {
        AttestationProof {
            exec_id,
            state: self.state,
            memory_used: self.memory_bytes,
            uptime: self.uptime().as_secs(),
            proof: self.generate_signed_snapshot(exec_id),
        }
    }
}
