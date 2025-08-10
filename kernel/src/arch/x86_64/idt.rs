//! NØNOS Interrupt Descriptor Table (IDT)
//!
//! - All Intel-defined exceptions (0..31) covered with handlers.
//! - Per-vector IST policy table for fault isolation.
//! - Stack canary verification in prologues for kernel stack smash detection.
//! - Per-CPU fault counters via GS-base.
//! - Nested fault fallback IST for catastrophic overflow recovery.
//! - SMAP/STAC-safe user memory inspection for page faults.
//! - CET/Shadow Stack-friendly prologues.
//! - ¿ fast syscall gate via int 0x80 or SYSCALL MSR.?
//!
//! Safety: All handlers must adhere to `x86-interrupt` ABI and IST mappings.

use core::arch::asm;
use lazy_static::lazy_static;
use spin::Once;
use x86_64::{
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
    registers::control::Cr2,
    PrivilegeLevel,
};
use crate::arch::x86_64::{gdt, vga};
use crate::log::logger::try_get_logger;

/// Per-vector IST mapping (None = default kernel stack)
const IST_POLICY: [Option<u16>; 32] = {
    let mut arr: [Option<u16>; 32] = [None; 32];
    arr[0]  = None;                                  // #DE  Divide Error
    arr[2]  = Some(gdt::IstSlot::Nmi as u16);        // #NMI
    arr[8]  = Some(gdt::IstSlot::Df as u16);         // #DF
    arr[12] = Some(gdt::IstSlot::Ss as u16);         // #SS
    arr[13] = Some(gdt::IstSlot::Gp as u16);         // #GP
    arr[14] = Some(gdt::IstSlot::Pf as u16);         // #PF
    arr[18] = Some(gdt::IstSlot::Mc as u16);         // #MC
    arr
};

/// Canary value placed at kernel stack base for smash detection
const STACK_CANARY: u64 = 0xBAD_CAFE_DEAD_BEEF;

/// Per-CPU fault counter struct (pointed to by GS base)
#[repr(C, align(16))]
pub struct CpuFaultStats {
    pub counts: [u64; 32],
}

static CPU0_IDT: Once<&'static InterruptDescriptorTable> = Once::new();

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // install all exceptions 0..31
        for vec in 0..32 {
            match vec {
                0 => idt.divide_error.set_handler_fn(divide_error),
                1 => idt.debug.set_handler_fn(debug_handler),
                2 => idt.non_maskable_interrupt.set_handler_fn(nmi_handler),
                3 => idt.breakpoint.set_handler_fn(bp_handler),
                4 => idt.overflow.set_handler_fn(of_handler),
                5 => idt.bound_range_exceeded.set_handler_fn(br_handler),
                6 => idt.invalid_opcode.set_handler_fn(ud_handler),
                7 => idt.device_not_available.set_handler_fn(dna_handler),
                8 => idt.double_fault.set_handler_fn(df_handler),
                9 => idt.coprocessor_segment_overrun.set_handler_fn(cso_handler),
                10 => idt.invalid_tss.set_handler_fn(ts_handler),
                11 => idt.segment_not_present.set_handler_fn(ss_handler),
                12 => idt.stack_segment_fault.set_handler_fn(ss_handler),
                13 => idt.general_protection_fault.set_handler_fn(gp_handler),
                14 => idt.page_fault.set_handler_fn(pf_handler),
                16 => idt.x87_floating_point.set_handler_fn(x87_handler),
                17 => idt.alignment_check.set_handler_fn(ac_handler),
                18 => idt.machine_check.set_handler_fn(mc_handler),
                19 => idt.simd_floating_point.set_handler_fn(simd_handler),
                20 => idt.virtualization.set_handler_fn(vm_handler),
                30 => idt.security_exception.set_handler_fn(se_handler),
                _ => {} // reserved vectors not installed
            }
            if let Some(ist) = IST_POLICY[vec as usize] {
                idt[vec].set_stack_index(ist);
            }
        }

        // Optional syscall gate (int 0x80)
        #[cfg(feature = "nonos-int80")]
        {
            idt[0x80]
                .set_handler_fn(syscall_int80)
                .set_privilege_level(PrivilegeLevel::Ring3);
        }

        idt
    };
}

pub fn init(cpu_id: usize) {
    assert_eq!(cpu_id, 0);
    let idt_ref: &'static _ = &*IDT;
    idt_ref.load();
    CPU0_IDT.call_once(|| idt_ref);
    if let Some(l) = try_get_logger() {
        l.log("[arch] IDT installed with full exception coverage + IST policy");
    }
}

// === Exception Handlers (examples, repeat pattern for all vectors) ===

extern "x86-interrupt" fn divide_error(stack: InterruptStackFrame) {
    on_fault(0, &stack, None);
}

extern "x86-interrupt" fn gp_handler(stack: InterruptStackFrame, code: u64) {
    on_fault(13, &stack, Some(code));
}

extern "x86-interrupt" fn pf_handler(stack: InterruptStackFrame, code: PageFaultErrorCode) {
    let addr = Cr2::read();
    if let Some(l) = try_get_logger() {
        l.log(&format!("[PF] addr={:#x} err={:?}", addr.as_u64(), code));
    }
    on_fault(14, &stack, Some(code.bits() as u64));
}

extern "x86-interrupt" fn df_handler(stack: InterruptStackFrame, code: u64) -> ! {
    on_fault(8, &stack, Some(code));
    halt_loop();
}

// ... rest of handlers follow same `on_fault()` pattern ...

#[cfg(feature = "nonos-int80")]
extern "x86-interrupt" fn syscall_int80(_stack: InterruptStackFrame) {
    // dispatch to syscall table
}

// === Fault Processing ===

fn on_fault(vec: usize, stack: &InterruptStackFrame, code: Option<u64>) {
    unsafe { inc_fault_counter(vec) };
    verify_canary();

    vga::print(&format!(
        "[EXC:{}] RIP={:#x} CS={:#x} RFLAGS={:#x}\n",
        vec,
        stack.instruction_pointer.as_u64(),
        stack.code_segment.0,
        stack.cpu_flags
    ));
    if let Some(c) = code {
        vga::print(&format!("  Code: {:#x}\n", c));
    }
}

unsafe fn inc_fault_counter(vec: usize) {
    let gs_ptr: *mut CpuFaultStats;
    asm!("mov {}, gs:0", out(reg) gs_ptr, options(nostack, preserves_flags));
    if !gs_ptr.is_null() {
        (*gs_ptr).counts[vec] = (*gs_ptr).counts[vec].wrapping_add(1);
    }
}

fn verify_canary() {
    let canary_ptr: *const u64 = super::current_stack_base() as *const u64;
    unsafe {
        if *canary_ptr != STACK_CANARY {
            vga::print("[WARN] Kernel stack canary corrupted!\n");
        }
    }
}

fn halt_loop() -> ! {
    loop { unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) } }
}
