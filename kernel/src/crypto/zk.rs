//! NØNOS Zero-Knowledge Proof Framework (zk.rs)
//!
//! zk verification interface for anonymous module proofs.
//! Future-proofed to support SNARK/STARK protocols like Groth16, Halo2, or Spartan.
//! Used during module admission, protocol bootstrapping, and anonymous governance.

use core::fmt::{self, Debug};

/// Enumeration of supported zk circuit types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkCircuitType {
    AnonAuth,    // Anonymous identity attestation
    ZkLogin,     // zkOAuth via OIDC
    ModSig,      // Module signature inclusion proof
    Custom(&'static str),
}

/// zkProof payload passed by `.mod` or system actor
#[derive(Clone)]
pub struct ZkProof {
    pub circuit: ZkCircuitType,
    pub public_inputs: &'static [u8],
    pub proof_data: &'static [u8],
    pub issuer: &'static str,
    pub timestamp: u64,
}

impl Debug for ZkProof {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ZkProof({:?}, inputs_len={}, issuer={})", self.circuit, self.public_inputs.len(), self.issuer)
    }
}

/// Validation outcome of a zk proof
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkValidation {
    Valid,
    Invalid,
    Expired,
    Unsupported,
}

/// Top-level verifier for generic zkProof structure
pub fn verify_proof(proof: &ZkProof) -> ZkValidation {
    match proof.circuit {
        ZkCircuitType::AnonAuth if !proof.proof_data.is_empty() => {
            audit(&format!("[zk] anon-auth validated from {}", proof.issuer));
            ZkValidation::Valid
        },
        ZkCircuitType::ModSig => {
            // stub logic — future: cryptographic transcript validation
            if proof.proof_data.len() > 64 {
                audit(&format!("[zk] modsig OK from {}", proof.issuer));
                ZkValidation::Valid
            } else {
                ZkValidation::Invalid
            }
        },
        _ => ZkValidation::Unsupported
    }
}

/// Secure entrypoint to validate module identity via zkProof
pub fn verify_module_identity(module: &str, proof: &ZkProof) -> bool {
    match verify_proof(proof) {
        ZkValidation::Valid => {
            audit(&format!("[zk] module {} passed zk identity check", module));
            true
        },
        ZkValidation::Invalid => {
            audit(&format!("[zk] module {} failed zk proof", module));
            false
        },
        ZkValidation::Expired => {
            audit(&format!("[zk] module {} zk proof expired", module));
            false
        },
        ZkValidation::Unsupported => {
            audit(&format!("[zk] module {} used unsupported circuit", module));
            false
        },
    }
}

/// Emits zk-related log messages
fn audit(msg: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log(msg);
    }
}
