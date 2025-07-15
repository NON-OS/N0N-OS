#![no_std]

/// Passed from bootloader ➞ kernel (always repr(C) ⇒ stable ABI).
#[repr(C)]
pub struct BootInfo<'a> {
    /// Physical offset where the kernel *thinks* phys 0 lives in its virt space.
    pub phys_memory_offset: u64,

    /// Slice covering the UEFI memory map copied into kernel space.
    pub memory_map: &'a [MemoryRegion],

    /// Root System-Description Pointer (ACPI 2+). None if firmware omitted it.
    pub rsdp_addr: Option<u64>,

    /// Raw command-line string given to the bootloader (e.g. from GRUB chainload).
    pub cmdline: &'a str,

    /// Address of kernel ELF in physical memory (for kexec/debugging).
    pub kernel_load_start: u64,
    pub kernel_load_size:  u64,
}

/// Mirror of uefi::table::boot::MemoryDescriptor but firmware-agnostic.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryRegion {
    pub start: u64,
    pub len:   u64,
    pub ty:    MemoryRegionType,
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum MemoryRegionType {
    Usable          = 1,
    Reserved        = 2,
    AcpiReclaimable = 3,
    AcpiNvs         = 4,
    Mmio            = 5,
    BadMemory       = 0xFFFF_FFFF,
}
