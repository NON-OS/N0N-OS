//! zkmeta.rs — NØNOS Capsule Metadata Parser 
//!
//! Parses embedded capsule headers containing cryptographic roots-of-trust
//! and zero-knowledge-compatible identifiers. This metadata enables:
//! - Secure zkVM commitment hashing
//! - Signature validation layout
//! - Capsule classification (boot/kernel/module)
//! - Deterministic replay prevention
//!
//! Used by the ZeroState bootloader and `verify.rs` during early-stage
//! capsule vetting. Failures result in hard boot rejection.

use core::convert::TryInto;
use sha2::{Digest, Sha256};
use alloc::vec::Vec;

/// Capsule metadata layout (hardcoded deterministic memory format)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct CapsuleMeta {
    pub magic: [u8; 4],           // "N0N\0" capsule marker
    pub version: u16,            // Metadata format version
    pub capsule_type: u8,        // 0 = boot, 1 = kernel, 2 = module
    pub flags: u8,               // Bitfield: 0x01 = ZK Required, 0x02 = Encrypted
    pub payload_len: u32,        // Binary region before signature
    pub zk_commit_hash: [u8; 32],// SHA-256 hash of payload (attested externally)
    pub sig_offset: u32,         // Byte offset where signature begins
    pub sig_len: u16,            // Byte length of RSA signature
    pub entropy: [u8; 16],       // Injected entropy for capsule-specific identity
    pub reserved: [u8; 4],       // Reserved (alignment/padding)
}

/// Capsule magic identifier used to validate capsule boundaries
pub const CAPSULE_MAGIC: &[u8; 4] = b"N0N\0";

/// Parses metadata header from raw capsule memory
pub fn parse_capsule_metadata(blob: &[u8]) -> Result<CapsuleMeta, &'static str> {
    if blob.len() < core::mem::size_of::<CapsuleMeta>() {
        return Err("Capsule header too short");
    }
    let meta: CapsuleMeta = unsafe {
        core::ptr::read_unaligned(blob.as_ptr() as *const CapsuleMeta)
    };
    if &meta.magic != CAPSULE_MAGIC {
        return Err("Invalid capsule magic tag");
    }
    Ok(meta)
}

/// Extracts detached RSA signature and payload from capsule
pub fn extract_signature_and_payload(blob: &[u8], meta: &CapsuleMeta) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    let sig_start = meta.sig_offset as usize;
    let sig_end = sig_start + (meta.sig_len as usize);
    if sig_end > blob.len() {
        return Err("Signature offset out of bounds");
    }
    let sig = blob[sig_start..sig_end].to_vec();
    let payload = blob[core::mem::size_of::<CapsuleMeta>()..sig_start].to_vec();
    Ok((sig, payload))
}

/// Generates a reproducible commitment hash from the capsule payload
/// Must match external zkVM commitment (Merkle root / zk-SNARK input)
pub fn compute_commitment(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).as_slice().try_into().unwrap()
}

/// Capsule classification types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CapsuleType {
    Bootloader,
    Kernel,
    Module,
    Unknown,
}

/// Resolves CapsuleType from metadata field
pub fn capsule_type(meta: &CapsuleMeta) -> CapsuleType {
    match meta.capsule_type {
        0 => CapsuleType::Bootloader,
        1 => CapsuleType::Kernel,
        2 => CapsuleType::Module,
        _ => CapsuleType::Unknown,
    }
}

/// Returns true if ZK commitment is required (hard enforcement)
pub fn requires_zk(meta: &CapsuleMeta) -> bool {
    meta.flags & 0x01 != 0
}

