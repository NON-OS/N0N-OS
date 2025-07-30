//! NØNOS Verified Module Manifest
//!
//! Provides verifiable execution metadata describing a .mod binary,
//! enforcing zero-trust policies, and supporting zk-authenticated modules.
//! Used during loading, validation, and runtime sandbox enforcement.

use crate::capabilities::Capability;
use crate::crypto::vault::{verify_signature, VaultPublicKey};
use crate::modules::runtime::FaultPolicy;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub enum AuthMethod {
    VaultSignature,
    ZkAttestation,
    HardwareRoot,
}

#[derive(Debug)]
pub struct ModuleManifest {
    pub name: &'static str,
    pub version: &'static str,

    // Core identity
    pub hash: [u8; 32],
    pub build_id: [u8; 32],
    pub entry_point_addr: Option<u64>,

    // Auth
    pub signature: [u8; 64],
    pub signer: VaultPublicKey,
    pub auth_chain_id: Option<[u8; 32]>,
    pub auth_method: AuthMethod,
    pub zk_attestation: Option<[u8; 64]>,

    // Capability contract
    pub required_caps: &'static [Capability],
    pub fault_policy: Option<FaultPolicy>,
    pub memory_bytes: usize,

    // Runtime validation
    pub timestamp: u64,
    pub expiry_seconds: Option<u64>,
}

impl ModuleManifest {
    /// Checks signature or proof based on declared method
    pub fn verify(&self) -> Result<(), &'static str> {
        match self.auth_method {
            AuthMethod::VaultSignature => {
                if verify_signature(&self.hash, &self.signature, &self.signer) {
                    Ok(())
                } else {
                    Err("Vault signature invalid")
                }
            },
            AuthMethod::ZkAttestation => {
                if let Some(proof) = self.zk_attestation {
                    // stub for future zk-verifier
                    if crate::crypto::zk::verify_proof(&self.hash, &proof) {
                        Ok(())
                    } else {
                        Err("ZK proof invalid")
                    }
                } else {
                    Err("Missing zk attestation payload")
                }
            },
            _ => Err("Unsupported authentication method"),
        }
    }

    /// Check manifest bounds and expiration logic
    pub fn validate_constraints(&self, now: u64) -> Result<(), &'static str> {
        if self.memory_bytes == 0 || self.memory_bytes > 64 * 1024 * 1024 {
            return Err("Manifest requested memory outside policy bounds");
        }
        if self.name.len() > 32 {
            return Err("Module name too long");
        }
        if let Some(expiry) = self.expiry_seconds {
            if now > self.timestamp + expiry {
                return Err("Manifest expired");
            }
        }
        Ok(())
    }
}
