//! NØNOS Advanced Memory Paging System
//!
//! This module provides a secure paging infrastructure compatible with the
//! ZeroState execution model. It sets up long mode paging using a recursive
//! level-4 structure, maps physical identity ranges for the kernel, and enables
//! future modular remapping for isolated `.mod` binaries. Integration with UEFI
//! memory descriptors enables dynamic frame allocation and bootloader coordination.

use x86_64::{
    VirtAddr, PhysAddr,
    structures::paging::{PageTable, PageTableFlags, OffsetPageTable, MapperAllSizes, FrameAllocator, Size4KiB, Page, PhysFrame},
};
use core::ptr::Unique;
use crate::memory::frame_alloc::BootFrameAllocator;

/// Virtual offset used for kernel-to-physical mapping (higher half mapping)
const PHYS_MEM_OFFSET: u64 = 0xFFFF800000000000;

/// Main paging initialization routine
///
/// - Maps kernel identity pages
/// - Enables OffsetPageTable abstraction
/// - Bootstraps future dynamic heap
pub fn init(mem_map: &[uefi::table::boot::MemoryDescriptor]) -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(PHYS_MEM_OFFSET);
    let level_4_table = unsafe { active_level_4_table(phys_offset) };
    let mut mapper = unsafe { OffsetPageTable::new(level_4_table, phys_offset) };

    let mut frame_alloc = unsafe { BootFrameAllocator::init_from_uefi(mem_map) };
    map_kernel_identity(&mut mapper, &mut frame_alloc);
    map_runtime_heap(&mut mapper, &mut frame_alloc);

    mapper
}

/// Extracts the active L4 table from CR3 and returns a writable reference
unsafe fn active_level_4_table(phys_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;
    let (frame, _) = Cr3::read();
    let phys = frame.start_address().as_u64();
    let virt = phys_offset + phys;
    let table_ptr = virt.as_mut_ptr::<PageTable>();
    &mut *table_ptr
}

/// Identity-maps the static kernel region using 4KiB pages
fn map_kernel_identity(mapper: &mut OffsetPageTable, allocator: &mut impl FrameAllocator<Size = Size4KiB>) {
    let start_phys = PhysAddr::new(0x100000); // 1 MiB
    let end_phys = PhysAddr::new(0x200000);   // 2 MiB (expand as needed)

    for frame_addr in (start_phys.as_u64()..end_phys.as_u64()).step_by(4096) {
        let frame = PhysFrame::containing_address(PhysAddr::new(frame_addr));
        let virt = VirtAddr::new(PHYS_MEM_OFFSET + frame_addr);
        let page = Page::containing_address(virt);

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        unsafe {
            mapper.map_to(page, frame, flags, allocator).expect("map failed").flush();
        }
    }
}

/// Maps heap memory used by the frame allocator (not `.mod` sandbox yet)
fn map_runtime_heap(mapper: &mut OffsetPageTable, allocator: &mut impl FrameAllocator<Size = Size4KiB>) {
    let heap_start = VirtAddr::new(0xFFFF_8800_0000_0000);
    let heap_size = 1024 * 1024; // 1 MiB runtime heap

    let heap_start_page = Page::containing_address(heap_start);
    let heap_end_page = Page::containing_address(heap_start + heap_size - 1u64);

    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        let frame = allocator.allocate_frame().expect("Heap frame allocation failed");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, allocator).expect("heap map failed").flush();
        }
    }
}
