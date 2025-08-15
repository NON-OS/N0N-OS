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

    /// Resolve the payload entry point as a pointer inside the payload slice.
    /// - If ELF64 (LE, x86_64), resolve e_entry → file offset via PT_LOAD mapping.
    /// - If not ELF, assume flat binary with offset 0.
    #[inline]
    pub fn entry_ptr(&self) -> Result<*const u8, &'static str> {
        let p = self.payload();
        let off = if is_elf64(p) {
            parse_elf_entry_offset(p)?
        } else {
            0usize
        };
        if off >= p.len() {
            return Err("entry offset out of bounds");
        }
        // SAFE: bounds-checked offset into payload slice
        Ok(unsafe { p.as_ptr().add(off) })
    }
} // end impl Capsule<'a>

/* ---------- ELF helpers (bounds-checked, unaligned reads) ---------- */

#[inline]
fn is_elf64(buf: &[u8]) -> bool {
    buf.len() >= 0x40
        && &buf[0..4] == b"\x7FELF"
        && buf[4] == 2 // 64-bit
        && buf[5] == 1 // little-endian
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;

/// Return FILE OFFSET of e_entry within the payload using the PT_LOAD that contains it.
fn parse_elf_entry_offset(buf: &[u8]) -> Result<usize, &'static str> {
    use core::{mem, ptr};
    if buf.len() < mem::size_of::<Elf64Ehdr>() {
        return Err("elf: header too small");
    }
    // SAFETY: unaligned read from checked slice
    let ehdr: Elf64Ehdr = unsafe { ptr::read_unaligned(buf.as_ptr() as *const _) };
    if ehdr.e_machine != EM_X86_64 {
        return Err("elf: wrong machine");
    }
    if ehdr.e_phentsize as usize != mem::size_of::<Elf64Phdr>() {
        return Err("elf: bad phentsize");
    }
    let phoff = ehdr.e_phoff as usize;
    let phnum = ehdr.e_phnum as usize;
    let phentsize = ehdr.e_phentsize as usize;
    let need = phoff
        .checked_add(
            phnum
                .checked_mul(phentsize)
                .ok_or("elf: phnum overflow")?,
        )
        .ok_or("elf: ph table overflow")?;
    if need > buf.len() {
        return Err("elf: ph table oob");
    }
    let entry = ehdr.e_entry;
    for i in 0..phnum {
        let off = phoff + i * phentsize;
        // SAFETY: bounds checked above
        let ph: Elf64Phdr = unsafe { ptr::read_unaligned(buf[off..].as_ptr() as *const _) };
        if ph.p_type != PT_LOAD {
            continue;
        }
        let vstart = ph.p_vaddr;
        let vend = ph
            .p_vaddr
            .checked_add(ph.p_filesz)
            .ok_or("elf: filesz overflow")?;
        if entry >= vstart && entry < vend {
            let rel = entry.checked_sub(vstart).ok_or("elf: entry underflow")?;
            let file_off = ph.p_offset.checked_add(rel).ok_or("elf: offset overflow")?;
            let file_off_usize = usize::try_from(file_off).map_err(|_| "elf: offset too large")?;
            if file_off_usize >= buf.len() {
                return Err("elf: entry offset oob");
            }
            return Ok(file_off_usize);
        }
    }
    Err("elf: entry not in any PT_LOAD")
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
