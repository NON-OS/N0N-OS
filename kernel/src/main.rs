//! NØNOS Freestanding Hardware Entrypoint (src/main.rs)
//!
//! This is the lowest level physical entry point for bare-metal execution.
//! Used only if booting without `bootloader` or UEFI handoff. It initializes
//! minimal console output and traps panics directly to VGA memory. This is
//! effectively the real-mode-to-long-mode bridge fallback before `lib.rs`
//! transitions into ZeroState runtime.

#![no_main]
#![no_std]
#![feature(panic_info_message, asm_const)]

use core::panic::PanicInfo;

/// Direct access to VGA text buffer (0xb8000) — early debugging only.
const VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;
const SCREEN_WIDTH: usize = 80;
const SCREEN_HEIGHT: usize = 25;
const BYTES_PER_CHAR: usize = 2;

/// Hardware startup entry point. This bypasses all runtime init and executes
/// directly from CPU reset vector into high-half kernel (or linked ELF).
#[no_mangle]
pub extern "C" fn _start() -> ! {
    vga_clear();
    vga_print("\n[NONOS: FREESTANDING MODE]\n");
    vga_print("RAM-Resident Bootloader Interface Initialized\n");
    vga_print("Awaiting secure jump to `kernel_main()`...\n");

    // Here we would normally jump to kernel_main() once paging & stack are valid
    loop {}
}

/// Prints a string to VGA text mode buffer
fn vga_print(msg: &str) {
    for (i, byte) in msg.bytes().enumerate() {
        unsafe {
            *VGA_BUFFER.offset((i * BYTES_PER_CHAR) as isize) = byte;
            *VGA_BUFFER.offset((i * BYTES_PER_CHAR + 1) as isize) = 0x0F; // white on black
        }
    }
}

/// Zero-clears the VGA console to black
fn vga_clear() {
    for i in 0..(SCREEN_WIDTH * SCREEN_HEIGHT) {
        unsafe {
            *VGA_BUFFER.offset((i * BYTES_PER_CHAR) as isize) = b' ';
            *VGA_BUFFER.offset((i * BYTES_PER_CHAR + 1) as isize) = 0x00;
        }
    }
}

/// Trap for panics occurring before the full kernel initializes
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga_print("\n\n[KERNEL PANIC]\n");
    if let Some(msg) = info.message() {
        vga_print(&format!("panic: {}\n", msg));
    } else {
        vga_print("panic: unknown\n");
    }
    loop {}
}
