//! zkmeta.rs — NØNOS Capsule Metadata Parser (hardened, bounds-checked)
// eK@nonos-tech.xyz
//
// EK | Dev notes:
// - Read header with `read_unaligned` (packed, C ABI).
// - Validate *all* layout invariants before exposing slices.
// - Use BLAKE3 (domain-separated) for commitments; keep SHA-256 helper if needed.
// - No RSA assumptions here; `verify.rs` decides static-vs-ZK path.
// - Constants and bitflags documented; tests cover overlap/bounds.
//
// On-wire layout (packed, little-endian):
//   struct CapsuleMeta {
//     u8  magic[4]      = "N0N\0";
//     u16 version       = 1;
//     u8  capsule_type  = 0:boot,1:kernel,2:module;
//     u8  flags         = bitfield (ZK_REQUIRED, ENCRYPTED);
//     u32 payload_len   = bytes of payload region (not incl. signature);
//     u8  zk_commit_hash[32];  // external commitment (optional, advisory)
//     u32 sig_offset;          // absolute offset of detached signature
//     u16 sig_len;             // length of signature/proof blob
//     u8  entropy[16];         // per-capsule salt (optional)
//     u8  reserved[4];
//   } // sizeof == 4+2+1+1+4+32+4+2+16+4 = 70 bytes

#![allow(dead_code)]

use core::{mem, ptr};
use core::convert::TryInto;
use alloc::vec::Vec;

use blake3;
use sha2::{Digest, Sha256}; // optional helper for interop

/// Capsule magic and version.
pub const CAPSULE_MAGIC: &[u8; 4] = b"N0N\0";
pub const CAPSULE_VERSION: u16 = 1;

/// Flags
pub const FLAG_ZK_REQUIRED: u8 = 1 << 0;
pub const FLAG_ENCRYPTED:  u8 = 1 << 1;

/// Capsule classification types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CapsuleType {
    Bootloader,
    Kernel,
    Module,
    Unknown,
}

/// Packed header as laid out in the capsule.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct CapsuleMeta {
    pub magic: [u8; 4],        // "N0N\0"
    pub version: u16,          // == CAPSULE_VERSION
    pub capsule_type: u8,      // 0 boot, 1 kernel, 2 module
    pub flags: u8,             // ZK_REQUIRED, ENCRYPTED
    pub payload_len: u32,      // bytes before signature
    pub zk_commit_hash: [u8; 32],
    pub sig_offset: u32,       // absolute offset to signature/proof
    pub sig_len: u16,          // signature/proof length
    pub entropy: [u8; 16],     // optional salt
    pub reserved: [u8; 4],     // future use
}

/// Parse header and run fast sanity (magic/version only). Layout validation is separate.
pub fn parse_capsule_metadata(blob: &[u8]) -> Result<CapsuleMeta, &'static str> {
    if blob.len() < mem::size_of::<CapsuleMeta>() {
        return Err("capsule header too short");
    }
    // SAFETY: unaligned read from checked slice
    let meta: CapsuleMeta = unsafe { ptr::read_unaligned(blob.as_ptr() as *const CapsuleMeta) };
    if &meta.magic != CAPSULE_MAGIC {
        return Err("bad capsule magic");
    }
    if meta.version != CAPSULE_VERSION {
        return Err("unsupported capsule metadata version");
    }
    Ok(meta)
}

/// Strong layout check: bounds, non-overlap, and payload_len consistency.
pub fn validate_capsule_layout(blob: &[u8], meta: &CapsuleMeta) -> Result<(), &'static str> {
    let hdr = mem::size_of::<CapsuleMeta>();
    let blob_len = blob.len();

    if (meta.payload_len as usize) < hdr {
        return Err("payload_len shorter than header");
    }

    // Payload spans [hdr .. sig_offset)
    let sig_start = meta.sig_offset as usize;
    let sig_len = meta.sig_len as usize;

    // Compute ends with overflow checks
    let payload_end = sig_start;
    if payload_end > blob_len { return Err("payload end oob"); }

    let sig_end = sig_start.checked_add(sig_len).ok_or("sig len overflow")?;
    if sig_end > blob_len { return Err("signature end oob"); }

    // The declared payload_len should match actual span (hdr..sig_start)
    let declared_payload_len = meta.payload_len as usize;
    let actual_payload_len = payload_end.checked_sub(hdr).ok_or("payload underflow")?;
    if declared_payload_len != actual_payload_len {
        return Err("payload_len mismatch vs sig_offset");
    }

    // Disallow zero sizes
    if declared_payload_len == 0 || sig_len == 0 {
        return Err("empty payload or signature");
    }

    Ok(())
}

/// Extract detached signature/proof and payload according to validated metadata.
/// Caller MUST have run `validate_capsule_layout` first (this fn rechecks bounds defensively).
pub fn extract_signature_and_payload(blob: &[u8], meta: &CapsuleMeta) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    validate_capsule_layout(blob, meta)?;

    let hdr = mem::size_of::<CapsuleMeta>();
    let sig_start = meta.sig_offset as usize;
    let sig_end = sig_start + (meta.sig_len as usize);

    let sig = blob[sig_start..sig_end].to_vec();
    let payload = blob[hdr..sig_start].to_vec();

    Ok((sig, payload))
}

/// BLAKE3 domain-separated commitment of payload (preferred).
pub fn compute_commitment(payload: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("NONOS:CAPSULE:COMMITMENT:v1");
    h.update(payload);
    *h.finalize().as_bytes()
}

/// Optional SHA-256 helper for compatibility paths.
pub fn compute_sha256(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).as_slice().try_into().unwrap()
}

/// Resolve CapsuleType from metadata field.
pub fn capsule_type(meta: &CapsuleMeta) -> CapsuleType {
    match meta.capsule_type {
        0 => CapsuleType::Bootloader,
        1 => CapsuleType::Kernel,
        2 => CapsuleType::Module,
        _ => CapsuleType::Unknown,
    }
}

/// Returns true if ZK commitment/proof is required (hard enforcement).
#[inline]
pub fn requires_zk(meta: &CapsuleMeta) -> bool {
    (meta.flags & FLAG_ZK_REQUIRED) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commitment_differs() {
        assert_ne!(compute_commitment(b"a"), compute_commitment(b"b"));
    }

    #[test]
    fn layout_ok() {
        // Build a tiny fake blob: [header][payload(8)][sig(4)]
        let hdr_len = core::mem::size_of::<CapsuleMeta>();
        let mut blob = vec![0u8; hdr_len + 8 + 4];

        // craft header
        let mut meta = CapsuleMeta {
            magic: *CAPSULE_MAGIC,
            version: CAPSULE_VERSION,
            capsule_type: 1,
            flags: 0,
            payload_len: 8 + hdr_len as u32,
            zk_commit_hash: [0u8;32],
            sig_offset: (hdr_len + 8) as u32,
            sig_len: 4,
            entropy: [0u8;16],
            reserved: [0u8;4],
        };
        // write header unaligned
        unsafe {
            let p = blob.as_mut_ptr() as *mut CapsuleMeta;
            ptr::write_unaligned(p, meta);
        }
        assert!(validate_capsule_layout(&blob, &meta).is_ok());
        let (_sig, payload) = extract_signature_and_payload(&blob, &meta).unwrap();
        assert_eq!(payload.len(), 8);
    }

    #[test]
    fn layout_bad_sig_oob() {
        let hdr_len = core::mem::size_of::<CapsuleMeta>();
        let mut blob = vec![0u8; hdr_len + 8 + 2];
        let meta = CapsuleMeta {
            magic: *CAPSULE_MAGIC,
            version: CAPSULE_VERSION,
            capsule_type: 1,
            flags: 0,
            payload_len: 8 + hdr_len as u32,
            zk_commit_hash: [0u8;32],
            sig_offset: (hdr_len + 8 + 1) as u32, // 1 byte inside sig, will oob
            sig_len: 4,
            entropy: [0u8;16],
            reserved: [0u8;4],
        };
        unsafe { ptr::write_unaligned(blob.as_mut_ptr() as *mut CapsuleMeta, meta); }
        assert!(validate_capsule_layout(&blob, &meta).is_err());
    }
}
