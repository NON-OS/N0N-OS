//! NØNOS Module Sandbox Enforcement
//!
//! Provides in-memory execution boundaries for sandboxed `.mod` runtimes.
//! Each module is associated with capability tokens and memory region guards.
//! This engine enforces syscall scope, memory safety, and runtime policy.

use crate::modules::registry::{ModInstance, ModRuntimeState, find_module};
use crate::syscall::capabilities::{Capability, CapabilityToken};
use crate::memory::region::{region_type_of, RegionType};
use x86_64::PhysAddr;

/// Result of a sandbox enforcement check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxDecision {
    Allowed,
    Denied(&'static str),
    Killed(&'static str),
}

/// Checks if a syscall can be invoked by a given module token
pub fn validate_syscall_scope(token: &CapabilityToken, required: Capability) -> SandboxDecision {
    if token.permissions.contains(&required) {
        SandboxDecision::Allowed
    } else {
        SandboxDecision::Denied("Unauthorized syscall access")
    }
}

/// Verifies that an address access is allowed by the current sandbox model
pub fn check_memory_access(token: &CapabilityToken, addr: PhysAddr) -> SandboxDecision {
    match region_type_of(addr) {
        Some(RegionType::ModBinary | RegionType::Heap | RegionType::Stack) => SandboxDecision::Allowed,
        Some(rt) => SandboxDecision::Denied("Access to restricted memory region"),
        None => SandboxDecision::Killed("Illegal memory access outside known regions"),
    }
}

/// Enforce runtime state before syscall or IPC
pub fn check_runtime_state(mod_name: &str) -> SandboxDecision {
    match find_module(mod_name) {
        Some(modref) => match modref.state {
            ModRuntimeState::Running => SandboxDecision::Allowed,
            ModRuntimeState::Suspended => SandboxDecision::Denied("Module suspended"),
            ModRuntimeState::Crashed | ModRuntimeState::Terminated => SandboxDecision::Killed("Dead module invoked syscall"),
            _ => SandboxDecision::Denied("Module not in runnable state"),
        },
        None => SandboxDecision::Killed("Unknown module token")
    }
}

/// Logs and isolates a misbehaving module
pub fn trigger_sandbox_violation(mod_name: &str, reason: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log("[SANDBOX] VIOLATION: ");
        logger.log(&format!("{} => {}", mod_name, reason));
    }
    // TODO: scheduler.mark_as_crashed(mod_name)
    // TODO: record violation in audit log
}

/// Audit hook for post-crash analysis
pub fn audit_crash(mod_name: &str) {
    // In production, snapshot heap/mem/syscall before kill
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log("[SANDBOX] CRASHED MODULE: ");
        logger.log(mod_name);
    }
}
