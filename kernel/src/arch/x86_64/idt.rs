use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        IDT.breakpoint.set_handler_fn(breakpoint_handler);
        IDT.load();
    }
}
extern "x86-interrupt" fn breakpoint_handler(_stack: InterruptStackFrame) {
    crate::arch::x86_64::vga::print("[BP]");
}
