//! verify.rs — NØNOS Capsule Verification Pipeline 
//!
//! This module performs capsule verification using dynamic backends:
//! - Zero-Knowledge Proof (ZKP) validation via pluggable verifiers
//! - Static RSA cryptographic checks
//! - Capsule header parsing and commitment validation

use crate::zk::zkverify::{ZkProof, ZkVerifyResult, verify_proof};
use crate::crypto::sig::verify_signature;
use crate::log::logger::{log_info, log_warn};
use crate::capsule::zkmeta::{requires_zk};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub enum CapsuleVerification {
    StaticVerified,
    ZkVerified,
    Failed(&'static str),
}

pub struct CapsuleMetadata {
    pub version: u8,
    pub flags: u8,
    pub offset_sig: usize,
    pub offset_payload: usize,
    pub len_sig: usize,
    pub len_payload: usize,
}

/// Primary capsule verification entry point
pub fn verify_capsule(blob: &[u8], meta: &CapsuleMetadata) -> CapsuleVerification {
    if requires_zk(meta) {
        match extract_zk_proof(blob, meta) {
            Ok(proof) => match verify_proof(&proof) {
                ZkVerifyResult::Valid => {
                    log_info("verify", "ZK proof accepted");
                    CapsuleVerification::ZkVerified
                }
                ZkVerifyResult::Invalid(e) | ZkVerifyResult::Unsupported(e) | ZkVerifyResult::Error(e) => {
                    log_warn("verify", e);
                    CapsuleVerification::Failed(e)
                }
            },
            Err(e) => CapsuleVerification::Failed(e),
        }
    } else {
        if verify_signature(blob, meta) {
            log_info("verify", "Static RSA signature passed");
            CapsuleVerification::StaticVerified
        } else {
            CapsuleVerification::Failed("RSA signature verification failed")
        }
    }
}

/// Construct ZkProof from metadata and blob
fn extract_zk_proof(blob: &[u8], meta: &CapsuleMetadata) -> Result<ZkProof, &'static str> {
    let sig_start = meta.offset_sig;
    let sig_end = sig_start + meta.len_sig;
    let payload_start = meta.offset_payload;
    let payload_end = payload_start + meta.len_payload;

    if sig_end > blob.len() || payload_end > blob.len() {
        return Err("Invalid capsule offsets");
    }

    let sig_blob = &blob[sig_start..sig_end];
    let capsule_payload = &blob[payload_start..payload_end];
    let commitment = sha256(capsule_payload);

    Ok(ZkProof {
        proof_blob: sig_blob.to_vec(),
        public_inputs: capsule_payload.to_vec(),
        program_hash: known_program_hash(),
        capsule_commitment: commitment,
    })
}

/// Predefined program hash for dev boot zkVM
fn known_program_hash() -> [u8; 32] {
    Sha256::digest(b"zkmod-attestation-program-v1").into()
}

/// Hash utility — returns 32-byte SHA-256 digest
fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

