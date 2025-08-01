//! NØNOS Preboot Entropy Generator — Hardened Capsule Seeder
//!
//! Gathers hardware-derived entropy during UEFI execution to securely seed
//! the ZeroState kernel's PRNG and cryptographic subsystems.
//!
//! This data is injected into `ZeroStateBootInfo.entropy` for deterministic
//! boot-time randomness across RAM-resident ephemeral sessions.
//!
//! ## Current Sources:
//! - TSC (timestamp counter) jitter via Stall microdelays
//! - High-resolution nanosecond RTC entropy
//! - Platform jitter over 64+ cycles
//!
//! ## Future Extensions:
//! - TPM 2.0 RNG or EFI_RNG_PROTOCOL
//! - RDRAND / RDSEED with fallback safety checks
//! - Peripheral entropy via input device UEFI events

use uefi::table::boot::BootServices;
use uefi_services::system_table;
use core::time::Duration;
use crate::handoff::ZeroStateBootInfo;

/// Collect a hardened entropy pool from CPU + RTC sources
pub fn collect_boot_entropy(bs: &BootServices) -> [u8; 64] {
    let mut entropy = [0u8; 64];
    let mut mix: u64 = 0xA5A5_5A5A_DEADBEEF;

    for round in 0..128 {
        let t1 = unsafe { core::arch::x86_64::_rdtsc() };
        bs.stall(29 + ((round * 7) % 13));
        let t2 = unsafe { core::arch::x86_64::_rdtsc() };

        let delta = t2.wrapping_sub(t1) ^ mix;
        mix = mix.rotate_left((round % 19) as u32) ^ delta;

        entropy[round % 64] ^= (delta >> (round % 8)) as u8;
    }

    if let Ok(rtc) = system_table().runtime_services().get_time() {
        let nano = rtc.nanosecond as u64;
        let sec = rtc.second as u64;
        let rtc_mix = ((nano << 16) | sec) ^ mix;

        for i in 0..64 {
            entropy[i] ^= ((rtc_mix >> (i % 56)) & 0xFF) as u8;
        }
    }

    entropy
}

/// Populate entropy field in `ZeroStateBootInfo` capsule struct
pub fn seed_entropy(info: &mut ZeroStateBootInfo, bs: &BootServices) {
    let collected = collect_boot_entropy(bs);
    info.entropy.copy_from_slice(&collected);
}

