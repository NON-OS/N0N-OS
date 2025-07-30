//! NØNOS Zero-Trust Module Loader
//!
//! Accepts signed module manifests and securely prepares them for runtime admission.
//! This is a cryptographically enforced interface between trusted validation
//! and memory-isolated runtime execution.

use crate::modules::manifest::ModuleManifest;
use crate::modules::auth::{issue_attested_token, CapabilityAttestation};
use crate::crypto::zk::{derive_admission_uid};
use crate::memory::region::{MemoryRegion, allocate_region};
use crate::log::logger::{log_info, log_error};
use crate::capabilities::CapabilityToken;
use crate::time::now;

/// Errors that can occur during module verification or provisioning
#[derive(Debug)]
pub enum LoaderError {
    InvalidManifest(&'static str),
    TokenRejected(&'static str),
    AllocationFailure,
}

/// Immutable admission record handed to runner and sandbox
pub struct ModuleAdmission {
    pub uid: [u8; 32],
    pub manifest: &'static ModuleManifest,
    pub attestation: CapabilityAttestation,
    pub memory: MemoryRegion,
}

impl ModuleAdmission {
    /// Export the assigned token
    pub fn token(&self) -> &CapabilityToken {
        &self.attestation.token
    }

    /// Generate a minimal audit snapshot
    pub fn audit_id(&self) -> [u8; 32] {
        self.uid
    }
}

/// Perform full manifest validation + memory provisioning
pub fn load_module(manifest: &'static ModuleManifest) -> Result<ModuleAdmission, LoaderError> {
    // Step 1: Manifest signature and timestamp validation
    manifest.verify().map_err(|e| LoaderError::InvalidManifest(e))?;
    manifest.validate_constraints(now()).map_err(|e| LoaderError::InvalidManifest(e))?;

    // Step 2: Capability attestation (vault or ZK)
    let attestation = issue_attested_token(manifest)
        .map_err(|e| LoaderError::TokenRejected("Capability attestation failed"))?;

    // Step 3: Secure memory provisioning
    let region = allocate_region(manifest.memory_bytes)
        .ok_or(LoaderError::AllocationFailure)?;

    // Step 4: Cryptographic UID for audit + relay
    let uid = derive_admission_uid(manifest, &attestation.token);

    // Step 5: Log admission
    log_info("mod_loader", &format!(
        "Admitted module '{}' | mem={} bytes | caps={:?} | uid={:x?}",
        manifest.name,
        manifest.memory_bytes,
        attestation.token.permissions,
        &uid[..6],
    ));

    Ok(ModuleAdmission {
        uid,
        manifest,
        attestation,
        memory: region,
    })
}
