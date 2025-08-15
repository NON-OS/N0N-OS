#![no_std]
#![no_main]

//! main.rs — NØNOS UEFI Boot Entrypoint
//! eK@nonos-tech.xyz
//
// Security-minded boot orchestration:
// - Early init of UEFI allocator/stdout for consistent logging.
// - Capsule is loaded & cryptographically verified before *any* jump.
// - No undefined behaviour from entry pointer: strict `usize` sanity check.
// - Explicit non-returning reset on failure (no dangling boot state).
// - Minimal unsafe: only for transmute+call into verified capsule.

use uefi::prelude::*;
use uefi::table::runtime::ResetType;
use uefi_services::init;

use crate::loader::load_kernel_capsule;
use crate::log::logger::{log_info, log_warn, log_critical};
use crate::handoff::ZeroStateBootInfo;

/// External capsule entry signature
type KernelEntry = extern "C" fn(*const ZeroStateBootInfo) -> !;

/// Entry point for UEFI firmware
#[entry]
fn efi_main(_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // 1. Init UEFI runtime: allocator, stdout, protocols
    if let Err(_) = init(&system_table) {
        fatal_reset(&system_table, "UEFI service initialization failed");
    }

    log_info("boot", "NØNOS Capsule Bootloader Activated");
    log_info("boot", "⚙ Initializing trustless capsule boot flow...");

    // 2. Load + verify capsule
    let kernel_capsule = match load_kernel_capsule(&system_table) {
        Ok(kc) => kc,
        Err(e) => {
            log_critical("boot", "[×] Capsule load/verify failed");
            log_warn("reason", e);
            fatal_reset(&system_table, "Capsule verification failed");
        }
    };

    log_info("capsule", "✓ Capsule validated, ready for execution");
    log_info("capsule", &format!(
        "⤴ Jumping to verified entry: 0x{:X} ({} bytes)",
        kernel_capsule.entry_point,
        kernel_capsule.size
    ));

    // 3. Guard: basic bounds check on entry_point inside capsule payload
    let base_addr = kernel_capsule.base as usize;
    let end_addr  = base_addr + kernel_capsule.size;
    if kernel_capsule.entry_point < base_addr || kernel_capsule.entry_point >= end_addr {
        fatal_reset(&system_table, "Entry point outside capsule bounds");
    }

    // 4. Convert raw usize to typed function pointer
    let kernel_entry: KernelEntry = unsafe {
        core::mem::transmute(kernel_capsule.entry_point)
    };

    // 5. Prepare handoff pointer (ZeroStateBootInfo telemetry)
    let handoff_ptr = &kernel_capsule.handoff as *const _;

    // 6. Transfer control — does not return
    unsafe {
        kernel_entry(handoff_ptr);
    }
}

/// Non-returning hard reset for boot failures
fn fatal_reset(st: &SystemTable<Boot>, reason: &str) -> ! {
    use core::ffi::c_void;

    log_warn("fatal", reason);
    let _ = st.stdout().reset(false);

    unsafe {
        st.runtime_services().reset(
            ResetType::Shutdown,
            Status::LOAD_ERROR,
            Some(reason as *const _ as *const c_void),
        );
    }

    loop {} // should never reach
}
