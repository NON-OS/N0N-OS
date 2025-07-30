//! NØNOS GDT (Global Descriptor Table) Initialization
//!
//! Establishes segmented memory model with kernel-only code/data protection,
//! integrates Task State Segment (TSS) for future interrupt stack isolation (IST),
//! and loads descriptors for CS, DS, and system-level trap handling.

use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::instructions::segmentation::{CS, Segment, SS};
use x86_64::instructions::tables::load_tss;
use lazy_static::lazy_static;
use x86_64::VirtAddr;

/// Index for double fault handler (future IST use)
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// IST stack size in bytes
pub const IST_STACK_SIZE: usize = 4096 * 5; // 20 KB

static mut IST_STACK: [u8; IST_STACK_SIZE] = [0; IST_STACK_SIZE];

lazy_static! {
    /// Allocated Task State Segment with future IST support
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        let stack_start = VirtAddr::from_ptr(unsafe { &IST_STACK });
        let stack_end = stack_start + IST_STACK_SIZE;
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stack_end;
        tss
    };

    /// Full GDT definition + segment selectors
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));

        (gdt, Selectors {
            code_selector,
            data_selector,
            tss_selector,
        })
    };
}

/// Segment selector structure
struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/// Install and activate the GDT in CPU state
pub fn init() {
    GDT.0.load();

    unsafe {
        CS::set_reg(GDT.1.code_selector);
        SS::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }

    crate::log::logger::try_get_logger().map(|logger| {
        logger.log("[ARCH] GDT loaded with kernel CS/DS/TSS and IST0 stack");
    });
}
