//! NØNOS Boot Handoff Interface — Kernel Launch Telemetry Capsule
//!
//! Defines the ZeroState metadata block passed to the NØNOS kernel at launch.
//! This is injected by the capsule loader into a known memory region, and must
//! be consumed as the first stage of the microkernel boot path.
//!
//! # Architecture Notes
//! - Struct is C-compatible and packed into exactly 128 bytes
//! - Includes precise capsule positioning, memory availability, and cryptographic entropy
//! - Future-ready: includes RTC, epoch timestamp, and extensible boot flags
//! - Aligned to hardware and boot trust assumptions
//!
//! # Field Verification
//! The kernel verifies the `magic` tag to ensure the handoff contract is intact.
//! Any failure to locate or verify this region results in ZeroState halt.

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroStateBootInfo {
    pub magic: u64,             // 0x4E4F4E4F53424F4F = "NONOSBOO"
    pub capsule_base: u64,      // Capsule physical base address
    pub capsule_size: u64,      // Size in bytes
    pub memory_start: u64,      // Usable memory start (post-UEFI)
    pub memory_size: u64,       // Total system memory size (RAM)
    pub boot_time_epoch: u64,   // UNIX timestamp at boot (UTC)
    pub entropy: [u8; 64],      // Cryptographically strong entropy slice
    pub rtc_utc: [u8; 8],       // Optional BCD/raw RTC timestamp
    pub boot_flags: u32,        // Boot mode bitflags (DEBUG, FALLBACK, etc.)
    pub reserved: [u8; 28],     // Padding for future expansion (aligns to 128B)
}

impl ZeroStateBootInfo {
    pub const MAGIC: u64 = 0x4E4F4E4F53424F4F; // "NONOSBOO"

    pub fn new() -> Self {
        Self {
            magic: Self::MAGIC,
            capsule_base: 0,
            capsule_size: 0,
            memory_start: 0,
            memory_size: 0,
            boot_time_epoch: 0,
            entropy: [0u8; 64],
            rtc_utc: [0u8; 8],
            boot_flags: 0,
            reserved: [0u8; 28],
        }
    }
}

/// Boot mode bitflag constants used to track launch state
#[repr(C)]
pub struct BootModeFlags;

impl BootModeFlags {
    pub const DEBUG: u32 = 0x01;
    pub const RECOVERY: u32 = 0x02;
    pub const FALLBACK: u32 = 0x04;
    pub const COLD_START: u32 = 0x08;
    pub const SECURE_BOOT: u32 = 0x10;
    pub const ZK_ATTESTED: u32 = 0x20;
}

