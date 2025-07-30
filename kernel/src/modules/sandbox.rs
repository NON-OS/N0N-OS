//! NØNOS Sandbox Context
//!
//! Provides per-module memory + capability + runtime isolation.
//! This is the foundation of zero-trust execution within NØNOS.

use crate::memory::region::{MemoryRegion, allocate_region};
use crate::capabilities::{CapabilityToken};
use crate::modules::runtime::{RuntimeCapsule, FaultPolicy};
use crate::log::logger::log_info;

use alloc::string::String;

/// Executable module sandbox — the execution perimeter of a `.mod` binary
pub struct SandboxContext {
    pub name: &'static str,
    pub memory: MemoryRegion,
    pub token: CapabilityToken,
    pub runtime: RuntimeCapsule,
}

impl SandboxContext {
    /// Create a new sandbox for an accepted module manifest
    pub fn new(name: &'static str, mem: MemoryRegion, token: &CapabilityToken) -> Result<Self, &'static str> {
        log_info("sandbox", &format!("Creating sandbox context for module '{}'", name));

        // Assign default fault policy — can be replaced by manifest later
        let policy = FaultPolicy::Restart;

        let runtime = RuntimeCapsule::new(name, token.clone(), policy, mem.size);

        Ok(Self {
            name,
            memory: mem,
            token: token.clone(),
            runtime,
        })
    }

    /// Query whether the module is still active
    pub fn is_alive(&self) -> bool {
        self.runtime.is_active()
    }

    /// Mutably access runtime capsule
    pub fn runtime_mut(&mut self) -> &mut RuntimeCapsule {
        &mut self.runtime
    }

    /// Immutable view of runtime capsule
    pub fn runtime(&self) -> &RuntimeCapsule {
        &self.runtime
    }

    /// Securely shut down this sandbox (future hook)
    pub fn shutdown(&mut self) {
        self.runtime.mark_inactive();
    }
}
