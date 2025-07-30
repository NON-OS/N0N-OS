//! NØNOS Capsule Runner 
//!
//! Converts trusted module admission records into isolated sandboxed runtime capsules.
//! Emits full audit trace, runtime attestation, and memory-scoped isolation.

use crate::modules::mod_loader::ModuleAdmission;
use crate::modules::sandbox::SandboxContext;
use crate::modules::registry::{register_module};
use crate::log::logger::{log_info, log_warn};
use crate::runtime::zerostate::{track_active_sandbox};
use crate::crypto::zk::{derive_exec_id, hash_capsule};
use crate::modules::auth::CapabilityAttestation;
use crate::capabilities::CapabilityToken;

/// Result of a verified module launch
#[derive(Debug)]
pub enum LaunchResult {
    Success(LaunchAudit),
    LaunchFailed(&'static str),
}

/// Cryptographic audit snapshot of a launched capsule
#[derive(Debug)]
pub struct LaunchAudit {
    pub module_name: &'static str,
    pub exec_id: [u8; 32],
    pub capsule_fingerprint: [u8; 32],
    pub token: CapabilityToken,
    pub memory_bytes: usize,
    pub attested: bool,
}

/// Launch and register a runtime capsule from admission
pub fn launch_module(admission: ModuleAdmission) -> LaunchResult {
    log_info("mod_runner", &format!(
        "Launching '{}' | caps: {:?} | mem: {} bytes",
        admission.manifest.name,
        admission.token().permissions,
        admission.memory.size
    ));

    let exec_id = derive_exec_id(admission.manifest.name, admission.token());

    // Step 1: Construct sandbox context
    let context = match SandboxContext::new(
        admission.manifest,
        admission.memory,
        admission.token(),
    ) {
        Ok(ctx) => ctx,
        Err(_) => return LaunchResult::LaunchFailed("Sandbox context failed"),
    };

    // Step 2: Track capsule in RAM
    track_active_sandbox(&context);

    // Step 3: Fingerprint capsule state for audit log
    let capsule_hash = hash_capsule(
        admission.manifest,
        admission.token(),
        context.memory.start_addr,
        context.memory.size,
    );

    // Step 4: Register to ZeroState registry
    register_module(
        admission.uid,
        admission.manifest,
        &context.runtime,
        exec_id,
        admission.attestation.signed_proof,
    );

    log_info("mod_runner", &format!(
        "Launched '{}' → exec_id={:x?} | fingerprint={:x?}",
        context.name,
        &exec_id[..6],
        &capsule_hash[..6],
    ));

    LaunchResult::Success(LaunchAudit {
        module_name: context.name,
        exec_id,
        capsule_fingerprint: capsule_hash,
        token: context.token.clone(),
        memory_bytes: context.memory.size,
        attested: true,
    })
}

/// Perform a dry-run capsule load for audit or bootstrap (no execution)
pub fn simulate_launch(admission: &ModuleAdmission) -> LaunchAudit {
    let exec_id = derive_exec_id(admission.manifest.name, admission.token());

    let capsule_hash = hash_capsule(
        admission.manifest,
        admission.token(),
        admission.memory.start_addr,
        admission.memory.size,
    );

    LaunchAudit {
        module_name: admission.manifest.name,
        exec_id,
        capsule_fingerprint: capsule_hash,
        token: admission.token().clone(),
        memory_bytes: admission.memory.size,
        attested: true,
    }
}
