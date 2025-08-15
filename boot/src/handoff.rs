//! handoff.rs — ZeroState Boot→Kernel ABI (stable, self-describing 128-byte block)
// eK@nonos-tech.xyz
//
// Design goals (read before editing):
// - EXACTLY 128 bytes, C-compatible, no interior pointers, safe to memcpy.
// - Contains ONLY what the kernel needs at time 0: capsule location, hash,
//   minimal memory map hints, entropy seed, boot flags, versioning.
// - Forward-compatible: `abi_version` + `hdr_size` allow additive evolution.
// - Cryptography: include a 32-byte capsule hash (BLAKE3/sha256—your choice upstream).
//
// IMPORTANT: We intentionally keep `entropy` at 32 bytes to hit 128 total.
// If your collector yields 64 bytes, TRUNCATE to 32 when populating this struct
// (keep the other 32 for DRBG reseed in boot code if you want).
//
// Layout (little-endian, packed):
//   0x00  u64 magic = "NONOSB00" (0x4F 0x30 0x30 0x42 0x53 0x4F 0x4E 0x4E) for easy hex grep
//   0x08  u16 abi_version (== 1)
//   0x0A  u16 hdr_size    (== 128)  // in bytes
//   0x0C  u32 boot_flags  (bitfield)
//   0x10  u64 capsule_base  (phys addr)
//   0x18  u64 capsule_size  (bytes)
//   0x20  u8  capsule_hash[32]      // commitment of payload
//   0x40  u64 memory_start  (first usable RAM after firmware)
//   0x48  u64 memory_size   (total RAM in bytes)
//   0x50  u8  entropy[32]            // seed material (truncate if you have 64)
//   0x70  u8  rtc_utc[8]             // optional RTC snapshot (YYMMDDhh or raw)
//   0x78  u8  reserved[8]            // kept zero; future use
// Total: 128 bytes

#![allow(dead_code)]

use core::{mem, ptr};

/// Magic tag "NONOSB00" in LE (for human-readable hex dumps).
pub const ZS_MAGIC: u64 = 0x30424F534F4E4F4E; // "NONOSB0" + "0"
pub const ZS_ABI_VERSION: u16 = 1;
pub const ZS_HDR_SIZE: u16 = 128;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroStateBootInfo {
    pub magic: u64,            // must == ZS_MAGIC
    pub abi_version: u16,      // must == ZS_ABI_VERSION
    pub hdr_size: u16,         // must == 128
    pub boot_flags: u32,       // BootModeFlags
    pub capsule_base: u64,     // physical base address of .mod blob
    pub capsule_size: u64,     // size in bytes
    pub capsule_hash: [u8; 32],// BLAKE3/SHA-256 commitment of payload
    pub memory_start: u64,     // first usable RAM (post-firmware)
    pub memory_size: u64,      // total RAM bytes
    pub entropy: [u8; 32],     // seed (truncate collector to 32 here)
    pub rtc_utc: [u8; 8],      // optional RTC snapshot (format up to you)
    pub reserved: [u8; 8],     // future fields (keep zero)
}

// Compile-time guard: assert exact size = 128
const _: () = {
    assert!(mem::size_of::<ZeroStateBootInfo>() == 128);
};

impl ZeroStateBootInfo {
    /// Create a zeroed, valid header with versioning fields set.
    #[inline]
    pub fn new() -> Self {
        Self {
            magic: ZS_MAGIC,
            abi_version: ZS_ABI_VERSION,
            hdr_size: ZS_HDR_SIZE,
            boot_flags: 0,
            capsule_base: 0,
            capsule_size: 0,
            capsule_hash: [0u8; 32],
            memory_start: 0,
            memory_size: 0,
            entropy: [0u8; 32],
            rtc_utc: [0u8; 8],
            reserved: [0u8; 8],
        }
    }

    /// Minimal invariant check the kernel should run before trusting fields.
    #[inline]
    pub fn basic_sanity(&self) -> bool {
        self.magic == ZS_MAGIC && self.abi_version == ZS_ABI_VERSION && self.hdr_size == ZS_HDR_SIZE
    }

    /// Safe memcpy into an out-parameter the kernel provides.
    /// (Useful if your boot code wants to place this at a known physical addr.)
    #[inline]
    pub unsafe fn copy_to(&self, dst: *mut ZeroStateBootInfo) {
        // structure is packed + POD; raw copy is fine
        ptr::copy_nonoverlapping(self as *const _, dst, 1);
    }
}

/// Boot mode bitflags (public API; keep in sync with kernel consumer)
#[repr(C)]
pub struct BootModeFlags;
impl BootModeFlags {
    pub const DEBUG:       u32 = 1 << 0;
    pub const RECOVERY:    u32 = 1 << 1;
    pub const FALLBACK:    u32 = 1 << 2;
    pub const COLD_START:  u32 = 1 << 3;
    pub const SECURE_BOOT: u32 = 1 << 4;
    pub const ZK_ATTESTED: u32 = 1 << 5;
}

/* -------------------------- Builder helpers (boot side) -------------------------- */

/// Fill the handoff block from components. Truncates `entropy64` to 32 bytes to fit header.
#[inline]
pub fn build_bootinfo(
    capsule_base: u64,
    capsule_size: u64,
    capsule_hash: [u8; 32],
    memory_start: u64,
    memory_size: u64,
    entropy64: &[u8; 64],
    rtc_utc: [u8; 8],
    boot_flags: u32,
) -> ZeroStateBootInfo {
    let mut info = ZeroStateBootInfo::new();
    info.capsule_base = capsule_base;
    info.capsule_size = capsule_size;
    info.capsule_hash = capsule_hash;
    info.memory_start = memory_start;
    info.memory_size = memory_size;
    info.boot_flags = boot_flags;
    info.rtc_utc = rtc_utc;
    info.entropy.copy_from_slice(&entropy64[..32]); // truncate to 32 to keep header 128B
    info
}

/* -------------------------- Kernel-side convenience -------------------------- */

impl ZeroStateBootInfo {
    /// Kernel helper: return (base, len) of capsule; safe to map/check.
    #[inline] pub fn capsule_span(&self) -> (u64, u64) { (self.capsule_base, self.capsule_size) }

    /// Kernel helper: true if header is sane and fields look non-zero-ish.
    #[inline]
    pub fn looks_populated(&self) -> bool {
        self.basic_sanity() &&
        self.capsule_size != 0 &&
        (self.capsule_base & 0xFFF) == 0 && // typically page-aligned
        self.memory_size > (16 * 1024 * 1024) // >16MiB sanity
    }
}
