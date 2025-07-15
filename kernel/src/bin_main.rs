#![no_main]
#![no_std]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let msg = b"NON-OS KERNEL: Secure RAM boot.\n";
    let vga = 0xb8000 as *mut u8;

    for (i, &byte) in msg.iter().enumerate() {
        unsafe { 
            *vga.offset(i as isize * 2) = byte;
        }
    }
    
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
