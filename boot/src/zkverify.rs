//! zkverify.rs — NØNOS Zero-Knowledge Capsule Verifier (hardened)
//! eK@nonos-tech.xyz
//
// EK | Dev notes:
// - Consistent commitments: use BLAKE3 (domain-separated), not ad-hoc SHA-256.
// - Strict size caps for proof/public inputs; reject absurd blobs early.
// - Constant-time comparisons for commitments/program IDs.
// - Feature-gated mock verifier for bring-up; defaults to *reject* without feature.
// - No panics; precise error strings for boot logs.

#![allow(dead_code)]

use alloc::vec::Vec;

use blake3;
use core::cmp::min;

/// Abstract proof type for any zk backend (SNARK, STARK, zkVM)
#[derive(Debug, Clone)]
pub struct ZkProof {
    /// Serialized proof bytes (backend-defined format)
    pub proof_blob: Vec<u8>,
    /// Public inputs (backend-defined encoding; typically CBOR/bytes)
    pub public_inputs: Vec<u8>,
    /// Hash of zkVM binary / circuit ID (domain-separated)
    pub program_hash: [u8; 32],
    /// Commitment from capsule payload (must match recomputation)
    pub capsule_commitment: [u8; 32],
}

/// Verification result for all supported ZK engines
#[derive(Debug, Clone, PartialEq)]
pub enum ZkVerifyResult {
    Valid,
    Invalid(&'static str),
    Unsupported(&'static str),
    Error(&'static str),
}

/* -------------------- constants & helpers -------------------- */

const DS_PROGRAM_HASH: &str = "NONOS:ZK:PROGRAM:v1";
const DS_COMMITMENT:   &str = "NONOS:CAPSULE:COMMITMENT:v1";

const MAX_PROOF_SIZE: usize = 2 * 1024 * 1024; // 2 MiB cap
const MAX_INPUT_SIZE: usize = 256 * 1024;      // 256 KiB cap

#[inline]
fn ct_eq32(a: &[u8;32], b: &[u8;32]) -> bool {
    let mut x = 0u8;
    for i in 0..32 { x |= a[i] ^ b[i]; }
    x == 0
}

#[inline]
fn blake3_commit(payload: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key(DS_COMMITMENT);
    h.update(payload);
    *h.finalize().as_bytes()
}

#[inline]
fn known_program_hash() -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key(DS_PROGRAM_HASH);
    h.update(b"zkmod-attestation-program-v1");
    *h.finalize().as_bytes()
}

/* -------------------- verifier entry -------------------- */

/// Verifies a Zero-Knowledge proof tied to a NØNOS capsule.
/// Checks:
/// 1) program identity (domain-separated hash),
/// 2) commitment binding (recompute from public inputs),
/// 3) backend proof verification or feature-gated mock.
///
/// Returns `Unsupported` if sizes are absurd or no backend is compiled in.
pub fn verify_proof(proof: &ZkProof) -> ZkVerifyResult {
    // quick size sanity
    if proof.proof_blob.len() > MAX_PROOF_SIZE {
        return ZkVerifyResult::Unsupported("proof too large");
    }
    if proof.public_inputs.len() > MAX_INPUT_SIZE {
        return ZkVerifyResult::Unsupported("public inputs too large");
    }

    // 1) program identity
    let prog = known_program_hash();
    if !ct_eq32(&proof.program_hash, &prog) {
        return ZkVerifyResult::Unsupported("unknown zk program hash");
    }

    // 2) binding: commitment(public_inputs) == capsule_commitment
    let local_commit = blake3_commit(&proof.public_inputs);
    if !ct_eq32(&local_commit, &proof.capsule_commitment) {
        return ZkVerifyResult::Invalid("commitment mismatch");
    }

    // 3) backend verify (feature-gated). By default, no backend.
    // - `mock-proof` feature: accept blobs that start with a fixed magic prefix.
    // - Real backends (halo2, risc0, etc.) should replace this block.
    #[cfg(feature = "mock-proof")]
    {
        const MAGIC: &[u8] = &[0xAA, 0xBB, 0xCC, 0xDD];
        let ok = proof.proof_blob.len() >= MAGIC.len()
            && &proof.proof_blob[..MAGIC.len()] == MAGIC;
        return if ok { ZkVerifyResult::Valid } else { ZkVerifyResult::Invalid("mock verifier: bad prefix") };
    }

    #[cfg(not(feature = "mock-proof"))]
    {
        ZkVerifyResult::Unsupported("no zk backend compiled")
    }
}

/* -------------------- dev/test helpers -------------------- */

/// Build a minimal test proof object for bring-up (`--features mock-proof`).
pub fn load_test_proof() -> ZkProof {
    let mut proof = Vec::with_capacity(8);
    // MAGIC prefix for mock verifier; pad a bit for realism
    proof.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 1, 2, 3, 4]);

    // Simple byte inputs; in real flows this is CBOR of public inputs.
    let inputs = vec![42, 43, 44, 45];

    ZkProof {
        proof_blob: proof,
        public_inputs: inputs.clone(),
        program_hash: known_program_hash(),
        capsule_commitment: blake3_commit(&inputs),
    }
}
