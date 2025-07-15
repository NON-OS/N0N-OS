use core::ptr::write_volatile;
const VGA: *mut u8 = 0xb8000 as *mut u8;

pub fn print(s: &str) {
    for (i, b) in s.bytes().enumerate() {
        unsafe {
            write_volatile(VGA.add(i * 2), b);
            write_volatile(VGA.add(i * 2 + 1), 0x0f);
        }
    }
}
