//! NØNOS Memory Region Management
//!
//! Provides memory region allocation and tracking for capsules and kernel subsystems.

use core::ptr::NonNull;
use x86_64::PhysAddr;

/// Memory region descriptor
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base: NonNull<u8>,
    pub size: usize,
    pub phys_base: PhysAddr,
    pub flags: RegionFlags,
}

bitflags::bitflags! {
    pub struct RegionFlags: u32 {
        const READABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const EXECUTABLE = 1 << 2;
        const USER = 1 << 3;
        const ZEROED = 1 << 4;
    }
}

impl MemoryRegion {
    pub fn new(base: NonNull<u8>, size: usize, phys: PhysAddr, flags: RegionFlags) -> Self {
        Self {
            base,
            size,
            phys_base: phys,
            flags,
        }
    }
    
    pub fn contains(&self, addr: usize) -> bool {
        let base = self.base.as_ptr() as usize;
        addr >= base && addr < base + self.size
    }
    
    pub fn size_bytes(&self) -> u64 {
        self.size as u64
    }
    
    pub fn zeroize(&self) {
        unsafe {
            core::ptr::write_bytes(self.base.as_ptr(), 0, self.size);
        }
    }
}

/// Allocate a memory region for a capsule
pub fn allocate_region(size: usize) -> Option<MemoryRegion> {
    use crate::memory::{phys, virt};
    use x86_64::{VirtAddr, PhysAddr};
    
    // Allocate physical frames
    let pages = (size + 4095) / 4096;
    let frame = phys::alloc_contig(pages, 1, phys::AllocFlags::ZERO)?;
    
    // Map to virtual memory
    extern "Rust" {
        fn __nonos_alloc_kvm_va(pages: usize) -> u64;
    }
    
    let va = unsafe { __nonos_alloc_kvm_va(pages) };
    if va == 0 {
        phys::free_contig(frame, pages);
        return None;
    }
    
    // Map pages
    for i in 0..pages {
        let page_va = VirtAddr::new(va + (i * 4096) as u64);
        let page_pa = PhysAddr::new(frame.0 + (i * 4096) as u64);
        
        unsafe {
            virt::map4k_at(
                page_va,
                page_pa,
                virt::VmFlags::RW | virt::VmFlags::NX | virt::VmFlags::GLOBAL
            ).ok()?;
        }
    }
    
    let base = NonNull::new(va as *mut u8)?;
    
    Some(MemoryRegion::new(
        base,
        size,
        PhysAddr::new(frame.0),
        RegionFlags::READABLE | RegionFlags::WRITABLE | RegionFlags::ZEROED
    ))
}

/// Free a memory region
pub fn free_region(region: &MemoryRegion) {
    use crate::memory::{phys, virt};
    use x86_64::VirtAddr;
    
    let pages = (region.size + 4095) / 4096;
    let va_base = region.base.as_ptr() as u64;
    
    // Unmap pages
    for i in 0..pages {
        let page_va = VirtAddr::new(va_base + (i * 4096) as u64);
        unsafe {
            let _ = virt::unmap4k(page_va);
        }
    }
    
    // Free physical frames
    phys::free_contig(phys::Frame(region.phys_base.as_u64()), pages);
}
