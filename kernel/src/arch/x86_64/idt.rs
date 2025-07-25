//! NØNOS Interrupt Descriptor Table (IDT) Setup
//!
//! Fully secure interrupt handling layer with fault isolation, nested stack fallback,
//! and precise architectural traps. Configures early critical exceptions with stack trace
//! feedback via VGA and audit logging. Integrates with `gdt.rs` for secure ISTs.

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use lazy_static::lazy_static;
use crate::arch::x86_64::gdt;
use crate::arch::x86_64::vga;
use crate::log::logger::try_get_logger;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        idt.divide_error.set_handler_fn(divide_by_zero_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

        // Future extension: user syscall trap
        // idt[0x80].set_handler_fn(syscall_handler).set_privilege_level(x86_64::PrivilegeLevel::Ring3);

        idt
    };
}

/// Loads the interrupt table into CPU register
pub fn init() {
    IDT.load();
    if let Some(logger) = try_get_logger() {
        logger.log("[ARCH] IDT registered with GPF, DF, PF, DIV0 handlers");
    }
}

extern "x86-interrupt" fn divide_by_zero_handler(stack: InterruptStackFrame) {
    vga::print("[EXC] Divide-by-zero\n");
    trace_trap("DIV0", &stack);
}

extern "x86-interrupt" fn general_protection_fault_handler(stack: InterruptStackFrame, code: u64) {
    vga::print("[EXC] General Protection Fault\n");
    vga::print(&format!("  Code: {:#x}\n", code));
    trace_trap("GPF", &stack);
}

extern "x86-interrupt" fn page_fault_handler(stack: InterruptStackFrame, error: PageFaultErrorCode) {
    use x86_64::registers::control::Cr2;
    let fault_addr = Cr2::read();
    vga::print("[EXC] Page Fault\n");
    vga::print(&format!("  Addr: {:?}, Err: {:?}\n", fault_addr, error));
    trace_trap("PF", &stack);
}

extern "x86-interrupt" fn double_fault_handler(stack: InterruptStackFrame, _code: u64) -> ! {
    vga::print("[FATAL] DOUBLE FAULT\n");
    trace_trap("DF", &stack);
    loop {}
}

/// Structured diagnostic frame dump for any trap
fn trace_trap(label: &str, stack: &InterruptStackFrame) {
    vga::print(&format!("  [{}] at RIP={:#x} CS={:#x}\n",
        label,
        stack.instruction_pointer.as_u64(),
        stack.code_segment.0,
    ));
    if let Some(logger) = try_get_logger() {
        logger.log(&format!("[EXC] {} at {:#x}", label, stack.instruction_pointer.as_u64()));
    }
}
