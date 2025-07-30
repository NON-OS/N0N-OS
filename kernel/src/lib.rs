//! NØNOS Kernel Entrypoint — Secure ZeroState Runtime
//!
//! This is the foundational entrypoint of the NØNOS operating system. It performs:
//! - Secure boot initialization (GDT, IDT, paging, heap)
//! - Root-of-trust provisioning via cryptographic vault
//! - Ephemeral ZeroState activation (RAM-only runtime)
//! - Modular sandbox subsystem and verified `.mod` loader
//! - Async-capable scheduler loop with syscalls and capability tokens

#![no_std]
#![no_main]
#![feature(alloc_error_handler, panic_info_message, lang_items)]

extern crate alloc;

// Subsystem modules
pub mod arch;
pub mod crypto;
pub mod ipc;
pub mod log;
pub mod memory;
pub mod modules;
pub mod runtime;
pub mod sched;
pub mod syscall;

// Imports
use core::panic::PanicInfo;
use arch::x86_64::{gdt, idt, vga};
use crypto::init_crypto;
use log::logger::init_logger;
use memory::{frame_alloc, heap};
use modules::mod_loader::{init_module_loader, load_core_module, ModuleLoadResult};
use runtime::zerostate::init_zerostate;
use sched::executor::run_scheduler;

/// Root kernel entry — executed by bootloader.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init_logger();
    log("\n[BOOT] NØNOS kernel starting...");

    // 1. Architecture bootstrap
    gdt::init();
    idt::init();
    log("[INIT] GDT/IDT initialized");

    // 2. Memory and allocator
    frame_alloc::init();
    heap::init_kernel_heap();
    log("[MEM] Heap and frame allocator initialized");

    // 3. Cryptographic root-of-trust
    init_crypto();
    assert!(crypto::crypto_ready(), "[SECURE] Vault failed to initialize");
    log("[SECURE] Cryptographic vault ready");

    // 4. ZeroState RAM runtime
    init_zerostate();
    log("[RUNTIME] ZeroState execution environment live");

    // 5. Module loader and secure manifest system
    init_module_loader();
    match load_core_module("core.boot") {
        ModuleLoadResult::Accepted(_) => log("[MOD] core.boot module registered"),
        ModuleLoadResult::Rejected(e) => log(&format!("[MOD] core.boot rejected: {}", e))
    }

    // 6. Async task scheduler
    log("[SCHED] Executor loop initialized");
    run_scheduler();
}

/// Trap any kernel panic and log failure reason.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga::print("\n💥 [PANIC]\n");
    if let Some(msg) = info.message() {
        vga::print(&format!("{}\n", msg));
    } else {
        vga::print("unknown panic\n");
    }
    loop {}
}

/// Trap allocator failures
#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    vga::print(&format!("\n❗ ALLOC ERROR: {:?}\n", layout));
    panic!("heap exhausted")
}

/// Lightweight early-stage logger
fn log(msg: &str) {
    vga::print(&format!("{}\n", msg));
}
