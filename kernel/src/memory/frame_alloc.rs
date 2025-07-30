//! NØNOS Physical Frame Allocator
//!
//! Provides physical memory frame allocation for the ZeroState runtime. Uses UEFI bootloader memory map
//! to establish ownership of `CONVENTIONAL` RAM regions, which are identity-safe and aligned for 4KiB paging.
//!
//! This allocator supports:
//! - Alignment-aware frame extraction
//! - Lazy bump-pointer strategy with multiple memory zones
//! - Integration with heap, paging, and module sandboxes
//! - Optional extension to buddy systems, slab, or zone-based policies

use core::ops::Range;
use spin::Mutex;
use x86_64::structures::paging::{PhysFrame, Size4KiB};
use x86_64::PhysAddr;

/// A range of physical memory available for frame allocation
#[derive(Debug, Clone)]
pub struct FrameRange {
    pub start: PhysAddr,
    pub end: PhysAddr,
}

impl FrameRange {
    /// Returns aligned next usable frame if available
    pub fn next_frame(&mut self) -> Option<PhysFrame> {
        let next_aligned = self.start.align_up(4096);
        if next_aligned + 4096u64 <= self.end {
            self.start = next_aligned + 4096u64;
            Some(PhysFrame::containing_address(next_aligned))
        } else {
            None
        }
    }
}

/// Core frame allocator managing physical memory pool
pub struct FrameAllocator {
    usable: Vec<FrameRange>,
    next: usize,
    frames_allocated: usize,
}

impl FrameAllocator {
    pub fn new() -> Self {
        FrameAllocator {
            usable: Vec::new(),
            next: 0,
            frames_allocated: 0,
        }
    }

    pub fn add_region(&mut self, start: PhysAddr, end: PhysAddr) {
        self.usable.push(FrameRange { start, end });
    }

    pub fn alloc(&mut self) -> Option<PhysFrame> {
        while self.next < self.usable.len() {
            if let Some(frame) = self.usable[self.next].next_frame() {
                self.frames_allocated += 1;
                return Some(frame);
            } else {
                self.next += 1;
            }
        }
        None
    }

    pub fn total_allocated(&self) -> usize {
        self.frames_allocated
    }

    pub fn regions_available(&self) -> usize {
        self.usable.len()
    }
}

lazy_static::lazy_static! {
    /// Singleton access to the global allocator instance
    pub static ref GLOBAL_ALLOCATOR: Mutex<FrameAllocator> = Mutex::new(FrameAllocator::new());
}

/// Initializes allocator from UEFI memory descriptors
pub fn init_from_map(mem_map: &[uefi::table::boot::MemoryDescriptor]) {
    let mut allocator = GLOBAL_ALLOCATOR.lock();
    for region in mem_map.iter() {
        if region.ty == uefi::table::boot::MemoryType::CONVENTIONAL {
            let start = PhysAddr::new(region.phys_start);
            let end = PhysAddr::new(region.phys_start + region.page_count * 4096);
            allocator.add_region(start, end);
        }
    }
    log_allocator_status("[ALLOC] Frame allocator initialized.");
}

/// Public allocation interface
pub fn alloc_frame() -> Option<PhysFrame> {
    GLOBAL_ALLOCATOR.lock().alloc()
}

/// Simple log interface (safe for early boot)
fn log_allocator_status(msg: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log(msg);
    } else {
        let vga = 0xb8000 as *mut u8;
        for (i, byte) in msg.bytes().enumerate().take(80) {
            unsafe {
                *vga.offset(i as isize * 2) = byte;
            }
        }
    }
}
