//! NØNOS Secure Kernel Initialization (lib.rs)
//!
//! This is the privileged execution root of the NØNOS microkernel.
//! It establishes the ZeroState execution context, secure memory setup,
//! cryptographic root-of-trust via Vault, module manifest system, and
//! async-capable scheduling loop. All runtime state remains RAM-resident,
//! governed by a capability-authenticated syscall interface.

#![no_std]
#![no_main]
#![feature(alloc_error_handler, panic_info_message, lang_items)]

extern crate alloc;

use core::panic::PanicInfo;
use crate::runtime::zerostate::init_zerostate;
use crate::modules::mod_loader::{init_module_loader, load_core_module};
use crate::sched::executor::run_scheduler;
use crate::log::logger::init_logger;
use crate::crypto::vault::{init_vault, is_vault_ready};
use crate::arch::x86_64::{gdt, idt, vga};
use crate::memory::frame_alloc::init as init_frame_alloc;

pub mod arch;
pub mod crypto;
pub mod ipc;
pub mod log;
pub mod memory;
pub mod modules;
pub mod runtime;
pub mod sched;
pub mod syscall;

/// Kernel entry function — bootloader invokes with full control at _start.
/// Performs layered subsystem initialization with explicit root-of-trust enforcement.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Stage 0: Logger (for VGA or serial-based diagnostics)
    init_logger();
    log("[BOOT] NØNOS kernel entrypoint reached");

    // Stage 1: CPU privilege descriptors and traps
    gdt::init();
    idt::init();
    log("[INIT] Arch subsystem: GDT/IDT initialized");

    // Stage 2: RAM frame allocator + memory subsystem
    init_frame_alloc();
    log("[MEM] Physical memory allocator initialized");

    // Stage 3: Crypto vault: device-sealed key provisioning
    init_vault();
    assert!(is_vault_ready(), "Vault initialization failed: root-of-trust unavailable");
    log("[SECURE] Vault integrity: OK");

    // Stage 4: ZeroState ephemeral boot environment
    init_zerostate();
    log("[RUNTIME] RAM-resident ZeroState active");

    // Stage 5: Modular loader (sandboxed `.mod` apps with capability auth)
    init_module_loader();
    let core_token = load_core_module("core.console");
    match core_token {
        crate::modules::mod_loader::ModuleLoadResult::Accepted(_) =>
            log("[MOD] core.console module registered"),
        _ => log("[MOD] core.console module failed to load")
    }

    // Stage 6: Secure scheduler and async dispatcher
    log("[SCHED] Launching task executor");
    run_scheduler();
}

/// Universal panic trap
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga::print("\n\n💥 [KERNEL PANIC]\n");
    if let Some(msg) = info.message() {
        vga::print(&format!("reason: {}\n", msg));
    } else {
        vga::print("reason: unknown\n");
    }
    loop {}
}

/// Kernel memory allocation failure trap
#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    vga::print(&format!("\n❗ [ALLOC ERROR] layout = {:?}\n", layout));
    panic!("heap exhausted")
}

/// Internal logger macro for VGA/debug UART output
fn log(msg: &str) {
    vga::print(&format!("{}\n", msg));
}
