//! NØNOS Module Capability Authentication
//!
//! Issues, verifies, and signs `CapabilityToken` contracts for use
//! in zero-trust syscall dispatch and runtime enforcement.

use crate::capabilities::{Capability, CapabilityToken};
use crate::modules::manifest::ModuleManifest;
use crate::crypto::zk::{generate_attestation, hash_token};
use crate::crypto::vault::{VaultPublicKey, sign_token};
use alloc::vec::Vec;

/// Capability issuance error types
#[derive(Debug)]
pub enum CapabilityIssueError {
    InvalidCapability,
    ForbiddenCombination,
    ChainMismatch,
    PolicyViolation(&'static str),
}

/// Capability policy validator (e.g. restrict CoreExec + Net together)
fn validate_capability_policy(requested: &[Capability]) -> Result<(), CapabilityIssueError> {
    if requested.is_empty() {
        return Err(CapabilityIssueError::InvalidCapability);
    }
    if requested.contains(&Capability::CoreExec) && requested.contains(&Capability::Net) {
        return Err(CapabilityIssueError::ForbiddenCombination);
    }
    Ok(())
}

/// Issue a verified capability token from a trusted manifest
pub fn issue_token(manifest: &ModuleManifest) -> Result<CapabilityToken, CapabilityIssueError> {
    validate_capability_policy(manifest.required_caps)?;

    // (Optional) enforce hardware/device chain origin
    if let Some(chain) = manifest.auth_chain_id {
        if !validate_auth_chain(chain) {
            return Err(CapabilityIssueError::ChainMismatch);
        }
    }

    Ok(CapabilityToken {
        owner_module: manifest.name,
        permissions: manifest.required_caps,
    })
}

/// Sign + attest token for runtime export
pub fn issue_attested_token(manifest: &ModuleManifest) -> Result<(CapabilityToken, [u8; 64]), CapabilityIssueError> {
    let token = issue_token(manifest)?;
    let hash = hash_token(&token); // ZK-friendly hash

    let sig = sign_token(&hash); // Trusted local vault
    Ok((token, sig))
}

/// Validate if token includes required capability
pub fn has_capability(token: &CapabilityToken, cap: Capability) -> bool {
    token.permissions.iter().any(|&c| c == cap)
}

/// Stub — enforce allowed origin of signing chain (e.g. enclave or zk circuit)
fn validate_auth_chain(_chain_id: [u8; 32]) -> bool {
    // In future: verify this against a trusted registry or ZK origin
    true
}
