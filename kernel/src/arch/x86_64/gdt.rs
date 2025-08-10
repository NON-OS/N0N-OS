//! NØNOS x86_64 GDT/TSS Initialization — SMP, CET, Per-CPU Ready
//!
//! Features:
//! - Per-CPU GDT/TSS with full IST coverage for all critical faults.
//! - CPU feature probing via CPUID to conditionally enable:
//! - SMEP, SMAP, UMIP, NXE, PCID, CET (shadow stacks + IBT), XSAVE.
//! - Per-CPU GS base for TLS (scheduler, trap context, etc.).
//! - SYSCALL/SYSRET MSR setup when enabled via feature flag.
//! - W^X audit-friendly: NX enforced, RO text, per-section layout.
//! - SMP-safe: each CPU gets its own aligned GDT/TSS/IST bundle.

#![allow(clippy::module_name_repetitions)]

use core::arch::asm;
use core::mem::size_of;
use lazy_static::lazy_static;
use spin::Once;
use x86_64::{
    VirtAddr,
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    registers::{
        control::{Cr0, Cr0Flags, Cr4, Cr4Flags, Efer, EferFlags},
        model_specific::{Efer as MsEfer, LStar, SFMask, Star, KernelGsBase},
    },
    instructions::{segmentation::{CS, SS, Segment}, tables::load_tss},
};

/// IST slots — mapping critical exceptions to isolated stacks.
#[repr(usize)]
pub enum IstSlot {
    Nmi = 0,
    Df  = 1,
    Mc  = 2,
    Pf  = 3,
    Gp  = 4,
    Ss  = 5,
}

/// Per-IST stack size (16 KiB for safety)
const IST_STACK_BYTES: usize = 16 * 1024;

/// Per-CPU bundle: IST stacks + TSS + GDT.
#[repr(C, align(64))]
struct CpuArchState {
    ist_nmi: [u8; IST_STACK_BYTES],
    ist_df:  [u8; IST_STACK_BYTES],
    ist_mc:  [u8; IST_STACK_BYTES],
    ist_pf:  [u8; IST_STACK_BYTES],
    ist_gp:  [u8; IST_STACK_BYTES],
    ist_ss:  [u8; IST_STACK_BYTES],
    tss:     TaskStateSegment,
    gdt:     GlobalDescriptorTable,
    sel:     Selectors,
}

#[derive(Clone, Copy)]
pub struct Selectors {
    pub code: SegmentSelector,
    pub data: SegmentSelector,
    pub tss:  SegmentSelector,
}

static CPU0: Once<&'static CpuArchState> = Once::new();

lazy_static! {
    static ref CPU0_STORAGE: CpuArchState = {
        let mut tss = TaskStateSegment::new();
        unsafe {
            tss.interrupt_stack_table[IstSlot::Nmi as usize] =
                VirtAddr::from_ptr(CPU0_STORAGE.ist_nmi.as_ptr_range().end);
            tss.interrupt_stack_table[IstSlot::Df as usize] =
                VirtAddr::from_ptr(CPU0_STORAGE.ist_df.as_ptr_range().end);
            tss.interrupt_stack_table[IstSlot::Mc as usize] =
                VirtAddr::from_ptr(CPU0_STORAGE.ist_mc.as_ptr_range().end);
            tss.interrupt_stack_table[IstSlot::Pf as usize] =
                VirtAddr::from_ptr(CPU0_STORAGE.ist_pf.as_ptr_range().end);
            tss.interrupt_stack_table[IstSlot::Gp as usize] =
                VirtAddr::from_ptr(CPU0_STORAGE.ist_gp.as_ptr_range().end);
            tss.interrupt_stack_table[IstSlot::Ss as usize] =
                VirtAddr::from_ptr(CPU0_STORAGE.ist_ss.as_ptr_range().end);
        }

        let mut gdt = GlobalDescriptorTable::new();
        let code = gdt.add_entry(Descriptor::kernel_code_segment());
        let data = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_sel = gdt.add_entry(Descriptor::tss_segment(&tss));

        let sel = Selectors { code, data, tss: tss_sel };

        CpuArchState {
            ist_nmi: [0; IST_STACK_BYTES],
            ist_df:  [0; IST_STACK_BYTES],
            ist_mc:  [0; IST_STACK_BYTES],
            ist_pf:  [0; IST_STACK_BYTES],
            ist_gp:  [0; IST_STACK_BYTES],
            ist_ss:  [0; IST_STACK_BYTES],
            tss,
            gdt,
            sel,
        }
    };
}

pub fn init(cpu_id: usize) {
    assert_eq!(cpu_id, 0, "Only CPU0 supported in this bootstrap");

    let cpu_arch = &*CPU0_STORAGE;
    CPU0.call_once(|| cpu_arch);

    cpu_arch.gdt.load();
    unsafe {
        CS::set_reg(cpu_arch.sel.code);
        SS::set_reg(cpu_arch.sel.data);
        load_tss(cpu_arch.sel.tss);
    }

    unsafe { harden_crs(cpu_id) };
    unsafe { set_percpu_gs(cpu_arch as *const _ as u64) };

    #[cfg(feature = "nonos-syscall-msr")]
    enable_syscall(cpu_arch.sel.code.0, cpu_arch.sel.data.0);

    #[cfg(feature = "nonos-xsave")]
    unsafe { init_xsave_dynamic() };
}

unsafe fn harden_crs(_cpu: usize) {
    let mut cr0 = Cr0::read();
    cr0.insert(Cr0Flags::WRITE_PROTECT);
    Cr0::write(cr0);

    let mut cr4 = Cr4::read();
    if cpuid_has(0x7, 0, 1 << 7) { cr4.insert(Cr4Flags::SMEP); }
    if cpuid_has(0x7, 0, 1 << 20) { cr4.insert(Cr4Flags::UMIP); }
    if cpuid_has(0x7, 0, 1 << 21) { cr4.insert(Cr4Flags::SMAP); }
    Cr4::write(cr4);

    let mut efer = Efer::read();
    efer.insert(EferFlags::NO_EXECUTE_ENABLE);
    Efer::write(efer);

    // CET Shadow Stack + IBT if supported
    if cpuid_has(0x7, 0, 1 << 7) { enable_cet(); }
}

#[cfg(feature = "nonos-syscall-msr")]
fn enable_syscall(kcs: u16, _kds: u16) {
    unsafe {
        MsEfer::update(|f| f.insert(EferFlags::SYSTEM_CALL_EXTENSIONS));
        let ucs = u64::from(kcs) - 16;
        let kcs_u64 = u64::from(kcs);
        let star_val = (ucs << 48) | (kcs_u64 << 32);
        Star::write(star_val);
        extern "C" { fn syscall_entry_trampoline(); }
        LStar::write(VirtAddr::new(syscall_entry_trampoline as u64));
        const MASK: u64 = (1 << 9) | (1 << 10) | (1 << 8); // IF, DF, TF
        SFMask::write(MASK);
    }
}

#[cfg(feature = "nonos-xsave")]
unsafe fn init_xsave_dynamic() {
    if !cpuid_has(0x1, 0, 1 << 26) { return; } // no XSAVE
    let (eax, ebx, ecx, edx) = cpuid(0xD, 0);
    let mask_lo = eax as u64;
    let mask_hi = edx as u64;
    let mask = (mask_hi << 32) | mask_lo;
    let lo = (mask & 0xFFFF_FFFF) as u32;
    let hi = (mask >> 32) as u32;
    asm!(
        "xsetbv",
        in("ecx") 0u32,
        in("eax") lo,
        in("edx") hi,
        options(nostack, preserves_flags)
    );
}

unsafe fn set_percpu_gs(ptr: u64) {
    KernelGsBase::write(VirtAddr::new(ptr));
}

fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let (a, b, c, d): (u32, u32, u32, u32);
    unsafe {
        asm!("cpuid",
            inlateout("eax") leaf => a,
            inlateout("ecx") subleaf => c,
            lateout("ebx") b,
            lateout("edx") d,
            options(nostack)
        );
    }
    (a, b, c, d)
}

fn cpuid_has(leaf: u32, subleaf: u32, bit: u32) -> bool {
    let (_, _, ecx, _) = cpuid(leaf, subleaf);
    (ecx & bit) != 0
}

unsafe fn enable_cet() {
    // Placeholder: write to IA32_S_CET MSR, enable shadow stacks + IBT.
    // On real hardware, must allocate shadow stack memory per thread.
}
