#![no_std]
#![no_main]

mod mem;
mod arch;
mod sched;
mod syscall;
mod crypto;

use core::panic::PanicInfo;
use crypto::measurement::BootInfo;
use arch::x86_64::{gdt, idt, vga};

#[no_mangle]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    gdt::init();
    idt::init();
    mem::init(info.mem_map);
    vga::print("🚀 NON-OS kernel online\n");
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::x86_64::vga::print("💥 kernel panic\n");
    arch::x86_64::vga::print(info.payload().downcast_ref::<&str>().unwrap_or(&""));
    loop {}
}
