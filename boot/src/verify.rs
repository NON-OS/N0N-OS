//! verify.rs — NØNOS Capsule Verification Pipeline (hardened)
// eK@nonos-tech.xyz

#![allow(dead_code)]

use alloc::vec::Vec;

use crate::capsule::zkmeta::requires_zk;
use crate::crypto::sig::verify_signature;          // back with ed25519 (recommended)
use crate::log::logger::{log_info, log_warn};
use crate::zk::zkverify::{verify_proof, ZkProof, ZkVerifyResult};

use blake3;
use sha2::{Digest, Sha256}; // optional if you still need SHA-256 elsewhere

/// Domain separation labels
const DS_CAPSULE_COMMIT: &str = "NONOS:CAPSULE:COMMITMENT:v1";
const DS_PROGRAM_HASH:   &str = "NONOS:ZK:PROGRAM:v1";

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
    if let Err(e) = validate_meta(blob, meta) {
        log_warn("verify", e);
        return CapsuleVerification::Failed(e);
    }

    if requires_zk(meta) {
        match extract_zk_proof(blob, meta) {
            Ok(proof) => match verify_proof(&proof) {
                ZkVerifyResult::Valid => {
                    log_info("verify", "ZK proof accepted");
                    CapsuleVerification::ZkVerified
                }
                ZkVerifyResult::Invalid(e)
                | ZkVerifyResult::Unsupported(e)
                | ZkVerifyResult::Error(e) => {
                    log_warn("verify", e);
                    CapsuleVerification::Failed(e)
                }
            },
            Err(e) => {
                log_warn("verify", e);
                CapsuleVerification::Failed(e)
            }
        }
    } else {
        if verify_signature(blob, meta) {
            log_info("verify", "Static signature accepted");
            CapsuleVerification::StaticVerified
        } else {
            CapsuleVerification::Failed("signature verification failed")
        }
    }
}

/// Construct ZkProof from metadata and blob
fn extract_zk_proof(blob: &[u8], meta: &CapsuleMetadata) -> Result<ZkProof, &'static str> {
    let (sig_blob, capsule_payload) = slices_for(blob, meta)?;

    let commitment = blake3_commit(capsule_payload);
    let prog_hash = known_program_hash();

    Ok(ZkProof {
        proof_blob: sig_blob.to_vec(),
        public_inputs: capsule_payload.to_vec(),
        program_hash: prog_hash,
        capsule_commitment: commitment,
    })
}

/// Compute capsule commitment (BLAKE3, domain-separated)
#[inline]
pub fn blake3_commit(payload: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key(DS_CAPSULE_COMMIT);
    h.update(payload);
    *h.finalize().as_bytes()
}

/// Decide if keep SHA-256 helper kept for compatibility
#[inline]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Stable program hash for dev boot zkVM (domain-separated BLAKE3).
/// Replace with Halo2 circuit ID hash when ready.
fn known_program_hash() -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key(DS_PROGRAM_HASH);
    h.update(b"zkmod-attestation-program-v1");
    *h.finalize().as_bytes()
}

/// Validate offsets and produce borrowed slices
#[inline]
fn slices_for<'a>(
    blob: &'a [u8],
    meta: &CapsuleMetadata,
) -> Result<(&'a [u8], &'a [u8]), &'static str> {
    let sig_start = meta.offset_sig;
    let sig_end = sig_start.checked_add(meta.len_sig).ok_or("sig len overflow")?;

    let pay_start = meta.offset_payload;
    let pay_end = pay_start
        .checked_add(meta.len_payload)
        .ok_or("payload len overflow")?;

    if sig_end > blob.len() || pay_end > blob.len() {
        return Err("offsets out of bounds");
    }
    if meta.len_sig == 0 || meta.len_payload == 0 {
        return Err("empty sig or payload");
    }

    // Disallow weird partial overlaps (allow equality if signature covers the whole payload)
    if ranges_overlap(sig_start, sig_end, pay_start, pay_end) && !(sig_start == pay_start && sig_end == pay_end) {
        return Err("sig/payload overlap");
    }

    Ok((&blob[sig_start..sig_end], &blob[pay_start..pay_end]))
}

/// Early metadata validation (lightweight)
#[inline]
fn validate_meta(blob: &[u8], meta: &CapsuleMetadata) -> Result<(), &'static str> {
    if blob.is_empty() {
        return Err("empty capsule blob");
    }
    if meta.len_sig > blob.len() || meta.len_payload > blob.len() {
        return Err("declared lengths exceed blob");
    }
    Ok(())
}

#[inline]
fn ranges_overlap(a0: usize, a1: usize, b0: usize, b1: usize) -> bool {
    a0 < b1 && b0 < a1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_is_32_bytes_and_changes() {
        let a = blake3_commit(b"hello");
        let b = blake3_commit(b"hello!");
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn meta_validation_catches_bounds() {
        let blob = [0u8; 64];
        let bad = CapsuleMetadata {
            version: 1,
            flags: 0,
            offset_sig: 60,
            len_sig: 8,
            offset_payload: 0,
            len_payload: 16,
        };
        assert!(validate_meta(&blob, &bad).is_err());
    }
}
