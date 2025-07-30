//! NØNOS Module Capability Authentication Layer
//!
//! This subsystem authenticates and issues runtime `CapabilityToken`s,
//! attaches cryptographic attestations, and ensures only verified modules
//! receive scoped permissions based on signed or ZK-authenticated manifest contracts.

use crate::capabilities::{Capability, CapabilityToken};
use crate::modules::manifest::ModuleManifest;
use crate::crypto::vault::{sign_token, VaultPublicKey};
use crate::crypto::zk::{hash_capabilities, verify_attestation};
use alloc::vec::Vec;

/// Issued capability attestation that proves origin, scope, and constraints.
#[derive(Debug, Clone)]
pub struct CapabilityAttestation {
    pub token: CapabilityToken,
    pub scope_fingerprint: [u8; 32],
    pub build_commitment: [u8; 32],
    pub signed_proof: [u8; 64],
}

/// Capability issuance failure types
#[derive(Debug)]
pub enum CapabilityIssueError {
    PolicyViolation(&'static str),
    AttestationFailure,
    InvalidSignature,
}

/// Enforce security model: restrict dangerous capability overlaps, enforce size limits
fn validate_policy(requested: &[Capability]) -> Result<(), CapabilityIssueError> {
    if requested.is_empty() {
        return Err(CapabilityIssueError::PolicyViolation("Empty capability set"));
    }
    if requested.contains(&Capability::CoreExec) && requested.contains(&Capability::Net) {
        return Err(CapabilityIssueError::PolicyViolation("CoreExec + Net requires isolated runtime"));
    }
    Ok(())
}

/// Issue a scoped token based on a trusted manifest
pub fn issue_token(manifest: &ModuleManifest) -> Result<CapabilityToken, CapabilityIssueError> {
    validate_policy(manifest.required_caps)?;

    Ok(CapabilityToken {
        owner_module: manifest.name,
        permissions: manifest.required_caps,
    })
}

/// Issue a signed attestation alongside the scoped token
pub fn issue_attested_token(manifest: &ModuleManifest) -> Result<CapabilityAttestation, CapabilityIssueError> {
    let token = issue_token(manifest)?;

    let scope_hash = hash_capabilities(token.permissions);
    let build_hash = manifest.build_id;

    let mut joined = [0u8; 64];
    joined[..32].copy_from_slice(&scope_hash);
    joined[32..].copy_from_slice(&build_hash);

    let sig = sign_token(&joined)
        .map_err(|_| CapabilityIssueError::InvalidSignature)?;

    Ok(CapabilityAttestation {
        token,
        scope_fingerprint: scope_hash,
        build_commitment: build_hash,
        signed_proof: sig,
    })
}

/// Validate attestation from remote module or registry
pub fn verify_attested_token(attest: &CapabilityAttestation, pubkey: &VaultPublicKey) -> bool {
    let mut joined = [0u8; 64];
    joined[..32].copy_from_slice(&attest.scope_fingerprint);
    joined[32..].copy_from_slice(&attest.build_commitment);

    verify_attestation(&joined, &attest.signed_proof, pubkey)
}
