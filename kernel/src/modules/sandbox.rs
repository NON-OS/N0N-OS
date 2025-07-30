//! NØNOS Sandbox Execution Context &Hardened 
//! core@dev|nonos
//! This is the isolated execution perimeter for any `.mod` binary. It enforces:
//! - Cryptographic execution identity (`exec_id`)
//! - Dynamic fault policy enforcement
//! - Runtime-bound memory fencing
//! - Capability-authenticated syscall perimeter
//! - Snapshot attestation export
//!
//! Every capsule is instantiated through this zero-trust boundary.

use crate::capabilities::{CapabilityToken};
use crate::crypto::zk::{AttestationProof, derive_exec_id, generate_snapshot_signature};
use crate::memory::region::{MemoryRegion, allocate_region};
use crate::modules::manifest::ModuleManifest;
use crate::modules::runtime::{RuntimeCapsule, FaultPolicy};
use crate::log::logger::{log_info, log_warn};

/// Core sandbox state encapsulation for a `.mod` capsule
pub struct SandboxContext {
    pub name: &'static str,
    pub exec_id: [u8; 32],
    pub memory: MemoryRegion,
    pub token: CapabilityToken,
    pub runtime: RuntimeCapsule,
}

impl SandboxContext {
    /// Construct a fully isolated sandbox from a manifest
    pub fn new(manifest: &'static ModuleManifest, token: &CapabilityToken) -> Result<Self, &'static str> {
        if !manifest.is_valid() {
            return Err("Manifest integrity or policy check failed");
        }

        let mem = allocate_region(manifest.memory_required as usize)
            .ok_or("Sandbox memory allocation failed")?;

        let exec_id = derive_exec_id(manifest.name, token);
        let policy = manifest.fault_policy.unwrap_or(FaultPolicy::Restart);
        let runtime = RuntimeCapsule::new(manifest.name, token.clone(), policy, mem.size);

        log_info("sandbox", &format!(
            "[+] Sandbox '{}' | exec_id={:x?} | cap_len={} | mem={} KB",
            manifest.name,
            &exec_id[..4],
            token.permissions.len(),
            mem.size / 1024
        ));

        Ok(Self {
            name: manifest.name,
            exec_id,
            memory: mem,
            token: token.clone(),
            runtime,
        })
    }

    /// Trigger a secure runtime halt
    pub fn shutdown(&mut self) {
        log_warn("sandbox", &format!("Shutting down '{}'", self.name));
        self.runtime.terminate();
        // In a production scenario, memory wipe and zeroization should occur here
        self.memory.zeroize();
    }

    /// Tick capsule runtime — invoked on IPC or CPU cycles
    pub fn tick(&mut self) {
        self.runtime.tick();
    }

    /// Check liveness
    pub fn is_active(&self) -> bool {
        self.runtime.is_active()
    }

    /// Enforce fault policy immediately (used on traps)
    pub fn enforce_fault(&mut self) {
        self.runtime.fault();
    }

    /// Immutable access to runtime telemetry
    pub fn runtime(&self) -> &RuntimeCapsule {
        &self.runtime
    }

    /// Mutable access for direct mutation
    pub fn runtime_mut(&mut self) -> &mut RuntimeCapsule {
        &mut self.runtime
    }

    /// Retrieve cryptographic capsule proof
    pub fn export_attestation(&self) -> AttestationProof {
        self.runtime.attestation(self.exec_id)
    }

    /// Retrieve the capsule execution ID
    pub fn exec_id(&self) -> [u8; 32] {
        self.exec_id
    }

    /// Return a summary trace (to be piped into registry/log)
    pub fn trace_summary(&self) -> &'static str {
        self.name
    }
}
