//! NØNOS Runtime Capsule Registry
//!
//! This is the ZeroState live RAM registry of executing `.mod` capsules.
//! It enables auditability, capsule inspection, lifecycle tracing,
//! and future telemetry export via zkRelay.

use crate::modules::runtime::{RuntimeCapsule, CapsuleState};
use crate::modules::manifest::ModuleManifest;
use crate::log::logger::{log_info, log_warn};
use crate::crypto::zk::AttestationProof;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;
use core::time::Duration;

#[derive(Debug, Clone)]
pub struct CapsuleMetadata {
    pub uid: [u8; 32],
    pub name: &'static str,
    pub exec_id: [u8; 32],
    pub state: CapsuleState,
    pub manifest: &'static ModuleManifest,
    pub proof: Option<AttestationProof>,
    pub heartbeat: Duration,
    pub memory_usage: usize,
}

impl CapsuleMetadata {
    pub fn is_alive(&self) -> bool {
        matches!(self.state, CapsuleState::Active)
    }
}

static REGISTRY: RwLock<BTreeMap<[u8; 32], CapsuleMetadata>> = RwLock::new(BTreeMap::new());

/// Insert or update a module capsule entry in the registry
pub fn register_module(
    uid: [u8; 32],
    manifest: &'static ModuleManifest,
    capsule: &RuntimeCapsule,
    exec_id: [u8; 32],
    proof: Option<AttestationProof>,
) {
    let meta = CapsuleMetadata {
        uid,
        name: manifest.name,
        exec_id,
        manifest,
        state: capsule.state(),
        heartbeat: capsule.last_seen(),
        memory_usage: capsule.memory_bytes(),
        proof,
    };

    REGISTRY.write().insert(uid, meta);
    log_info("registry", &format!(
        "Registered module: '{}' | exec_id={:x?} | mem={} KB",
        manifest.name,
        &exec_id[..6],
        meta.memory_usage / 1024
    ));
}

/// Remove a module entry by UID
pub fn unregister_module(uid: &[u8; 32]) -> bool {
    let mut reg = REGISTRY.write();
    match reg.remove(uid) {
        Some(meta) => {
            log_warn("registry", &format!("Module '{}' unregistered", meta.name));
            true
        }
        None => false,
    }
}

/// List all live capsules in registry
pub fn list_capsules() -> Vec<CapsuleMetadata> {
    REGISTRY.read().values().cloned().collect()
}

/// Search for a capsule by exec_id
pub fn find_by_exec_id(exec_id: &[u8; 32]) -> Option<CapsuleMetadata> {
    REGISTRY.read().values().find(|m| m.exec_id == *exec_id).cloned()
}

/// Search for a capsule by name
pub fn find_by_name(name: &str) -> Option<CapsuleMetadata> {
    REGISTRY.read().values().find(|m| m.name == name).cloned()
}

/// Export audit snapshot (for CLI or telemetry relay)
pub fn export_snapshot() -> Vec<CapsuleMetadata> {
    list_capsules()
        .into_iter()
        .filter(|m| m.is_alive())
        .collect()
}

/// Return current capsule count
pub fn active_count() -> usize {
    REGISTRY.read().len()
}
