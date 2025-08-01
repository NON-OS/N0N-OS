#![no_std]
#![no_main]

use uefi::prelude::*;
use uefi::table::runtime::ResetType;
use uefi_services::init;

use crate::loader::load_kernel_capsule;
use crate::log::logger::{log_info, log_warn, log_critical};
use crate::capsule::Capsule;
use crate::handoff::ZeroStateBootInfo;

/// External capsule entry signature
type KernelEntry = extern "C" fn(*const ZeroStateBootInfo) -> !;

/// Entry point for UEFI firmware
#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // Initialize UEFI services (allocators, stdout, etc.)
    if let Err(e) = init(&system_table) {
        fatal_reset(&system_table, "UEFI service initialization failed");
    }

    log_info("boot", "NØNOS Capsule Bootloader Activated");
    log_info("boot", "⚙️  Initializing trustless capsule boot flow...");

    // Load + verify kernel capsule from disk (FS read + memory relocation)
    let kernel_capsule = match load_kernel_capsule(&system_table) {
        Ok(kc) => kc,
        Err(e) => {
            log_critical("boot", "[×] Capsule load/verify failed");
            log_warn("reason", e);
            fatal_reset(&system_table, "Capsule verification failed");
        }
    };

    // Log validated telemetry
    log_info("capsule", "✓ Capsule validated, ready for execution");
    log_info("capsule", &format!(
        "⤴️ Jumping to verified entry: 0x{:X} ({} bytes)",
        kernel_capsule.entry_point,
        kernel_capsule.size
    ));

    // Convert function pointer safely to typed trampoline
    let kernel_entry: KernelEntry =
        unsafe { core::mem::transmute(kernel_capsule.entry_point) };

    // Hand off ZeroStateBootInfo struct (entropy, memmap, etc.)
    let handoff_ptr = &kernel_capsule.handoff as *const _;

    // SAFETY: We verified the capsule cryptographically
    unsafe {
        kernel_entry(handoff_ptr);
    }
}

/// Non-returning hard reset path for failed boots
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

    loop {} // Should never reach
}

