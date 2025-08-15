//! capsule.rs — NØNOS Boot Capsule Handler (secure parsing + layout checks)
//! eK@nonos-tech.xyz
//
// Pragmatic, security-minded implementation:
// - Zero unsafe pointer aliasing: header is read with `read_unaligned`
// - Strict bounds + overlap validation for all offsets/lengths
// - Domain-separated BLAKE3 commitment for payload (fast + solid)
// - Clear invariants in comments; small helpers expose typed slices
//
// Layout (little-endian):
//   +----------------------+ 0
//   | magic = b"N0N\0"     | 4  (u8[4])
//   | version              | 1  (u8)
//   | flags                | 1  (u8)  e.g., ZK_REQUIRED, COMPRESSED
//   | offset_sig           | 4  (u32 LE)
//   | offset_payload       | 4  (u32 LE)
//   | len_sig              | 4  (u32 LE)
//   | len_payload          | 4  (u32 LE)
//   +----------------------+ sizeof(CapsuleHeader) == 22 bytes
//   | signature/proof ...  |
//   | payload bytes ...    |
//   +----------------------+
#![allow(dead_code)]

use core::{mem, ptr};
use alloc::vec::Vec;

use blake3;
use crate::log::logger::{log_info, log_warn};
use crate::verify::{verify_capsule, CapsuleVerification, CapsuleMetadata};

/// Magic and versioning
pub const CAPSULE_MAGIC: &[u8; 4] = b"N0N\0";
pub const CAPSULE_VERSION: u8 = 1;

/// Flags
pub const FLAG_ZK_REQUIRED: u8   = 1 << 0;
pub const FLAG_COMPRESSED: u8    = 1 << 1; // payload is compressed (decompress before exec)

/// On-wire header. Keep repr(C) and read it with `read_unaligned` from the blob.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CapsuleHeader {
    pub magic: [u8; 4],       // must be b"N0N\0"
    pub version: u8,          // format version (== CAPSULE_VERSION)
    pub flags: u8,            // FLAG_* bitfield
    pub offset_sig: u32,      // absolute byte offset to signature/proof
    pub offset_payload: u32,  // absolute byte offset to payload
    pub len_sig: u32,         // signature/proof length
    pub len_payload: u32,     // payload length
}

/// Runtime view over a capsule blob.
pub struct Capsule<'a> {
    pub header: CapsuleHeader,
    pub blob:   &'a [u8],
}

impl CapsuleHeader {
    /// Fast header sanity: magic + version only. Offsets/lengths are validated later.
    #[inline]
    pub fn basic_valid(&self) -> bool {
        &self.magic == CAPSULE_MAGIC && self.version == CAPSULE_VERSION
    }
}

impl<'a> Capsule<'a> {
    /// Parse a capsule from raw bytes. Performs:
    ///  - header presence
    ///  - unaligned read
    ///  - magic/version check
    ///  - full layout validation (bounds + overlap)
    pub fn from_blob(blob: &'a [u8]) -> Result<Self, &'static str> {
        let need = mem::size_of::<CapsuleHeader>();
        if blob.len() < need {
            return Err("blob too small for header");
        }

        // SAFETY: We only read `need` bytes from a valid slice; header may be unaligned.
        let header: CapsuleHeader = unsafe { ptr::read_unaligned(blob.as_ptr() as *const _) };

        if !header.basic_valid() {
            return Err("invalid capsule magic/version");
        }

        // Validate offsets/lengths (convert once to usize)
        let h = HeaderU {
            offset_sig:      header.offset_sig as usize,
            offset_payload:  header.offset_payload as usize,
            len_sig:         header.len_sig as usize,
            len_payload:     header.len_payload as usize,
        };
        validate_layout(blob.len(), &h)?;

        Ok(Self { header, blob })
    }

    /// Convert to the verifier metadata struct.
    #[inline]
    pub fn to_metadata(&self) -> CapsuleMetadata {
        CapsuleMetadata {
            version:        self.header.version,
            flags:          self.header.flags,
            offset_sig:     self.header.offset_sig as usize,
            offset_payload: self.header.offset_payload as usize,
            len_sig:        self.header.len_sig as usize,
            len_payload:    self.header.len_payload as usize,
        }
    }

    /// Signature/proof bytes slice (borrowed).
    #[inline]
    pub fn sig(&self) -> &'a [u8] {
        let s = self.header.offset_sig as usize;
        let e = s + self.header.len_sig as usize;
        &self.blob[s..e]
    }

    /// Payload bytes slice (borrowed).
    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        let s = self.header.offset_payload as usize;
        let e = s + self.header.len_payload as usize;
        &self.blob[s..e]
    }

    /// BLAKE3 commitment of payload (domain-separated).
    #[inline]
    pub fn commitment(&self) -> [u8; 32] {
        let mut h = blake3::Hasher::new_derive_key("NONOS:CAPSULE:COMMITMENT:v1");
        h.update(self.payload());
        *h.finalize().as_bytes()
    }

    /// Run full verification pipeline (ZK or static sig).
    pub fn verify(&self) -> CapsuleVerification {
        let meta = self.to_metadata();
        verify_capsule(self.blob, &meta)
    }

    /// Launch the payload (placeholder): this only logs success.
    /// Real launch maps the payload into ZeroState and transfers control.
    pub fn launch(&self) -> Result<(), &'static str> {
        match self.verify() {
            CapsuleVerification::StaticVerified | CapsuleVerification::ZkVerified => {
                let n = self.payload().len();
                log_info("capsule", &format!("payload verified ({} bytes), launching", n));
                // TODO(eK): map VMO, enforce policy (mem cap / cpu ns), jump to entry
                Ok(())
            }
            CapsuleVerification::Failed(e) => {
                log_warn("capsule", e);
                Err("capsule verification failed")
            }
        }
    }
}

/* ---------- internal helpers (pure, testable) ---------- */

#[derive(Copy, Clone)]
struct HeaderU {
    offset_sig: usize,
    offset_payload: usize,
    len_sig: usize,
    len_payload: usize,
}

/// Validate that all spans are inside blob and not malformed/overlapping.
/// Allowed: signature span == payload span (if signature is over entire payload blob).
fn validate_layout(blob_len: usize, h: &HeaderU) -> Result<(), &'static str> {
    // zero lengths are not allowed
    if h.len_sig == 0 || h.len_payload == 0 {
        return Err("empty sig or payload");
    }

    // compute ends with overflow checks
    let sig_end = h.offset_sig.checked_add(h.len_sig).ok_or("sig len overflow")?;
    let pay_end = h.offset_payload.checked_add(h.len_payload).ok_or("payload len overflow")?;

    // bounds
    if sig_end > blob_len || pay_end > blob_len {
        return Err("offsets out of bounds");
    }

    // overlap policy
    if ranges_overlap(h.offset_sig, sig_end, h.offset_payload, pay_end)
        && !(h.offset_sig == h.offset_payload && sig_end == pay_end)
    {
        return Err("sig/payload overlap");
    }

    // header must be fully contained (paranoia: ensure header region exists)
    if mem::size_of::<CapsuleHeader>() > blob_len {
        return Err("header truncated");
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
    fn layout_good() {
        let blob_len = 4096;
        let h = HeaderU {
            offset_sig: 64, len_sig: 128,
            offset_payload: 512, len_payload: 1024
        };
        assert!(validate_layout(blob_len, &h).is_ok());
    }

    #[test]
    fn layout_bounds() {
        let blob_len = 256;
        let h = HeaderU {
            offset_sig: 240, len_sig: 32,
            offset_payload: 0, len_payload: 128
        };
        assert!(validate_layout(blob_len, &h).is_err());
    }

    #[test]
    fn layout_overlap_rejected() {
        let blob_len = 2048;
        let h = HeaderU {
            offset_sig: 100, len_sig: 200,
            offset_payload: 250, len_payload: 300
        };
        assert!(validate_layout(blob_len, &h).is_err());
    }

    #[test]
    fn layout_equal_allowed() {
        let blob_len = 2048;
        let h = HeaderU {
            offset_sig: 256, len_sig: 512,
            offset_payload: 256, len_payload: 512
        };
        assert!(validate_layout(blob_len, &h).is_ok());
    }
}
