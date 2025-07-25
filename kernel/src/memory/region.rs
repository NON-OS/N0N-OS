//! NØNOS Memory Region Descriptors
//!
//! Provides type-safe classification of memory layout zones in the ZeroState runtime:
//! - Kernel-reserved memory
//! - Bootloader and metadata areas
//! - Heap, stack, and frame alloc domains
//! - Device MMIO zones
//!
//! Used for diagnostics, sandbox enforcement, and per-module memory accounting.

use x86_64::PhysAddr;

/// Enumerates the type of memory segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionType {
    KernelText,
    KernelData,
    Bootloader,
    Stack,
    Heap,
    FramePool,
    ModBinary,
    MMIO,
    Reserved,
}

/// Describes a memory region with type tag and physical bounds.
#[derive(Debug, Clone, Copy)]
pub struct MemRegion {
    pub start: PhysAddr,
    pub end: PhysAddr,
    pub region_type: RegionType,
    pub readonly: bool,
    pub executable: bool,
}

impl MemRegion {
    pub fn contains(&self, addr: PhysAddr) -> bool {
        addr >= self.start && addr < self.end
    }

    pub fn size_bytes(&self) -> u64 {
        self.end.as_u64() - self.start.as_u64()
    }
}

/// Central catalog of memory region layout (optional: build during ZeroState init)
static mut REGIONS: [Option<MemRegion>; 32] = [None; 32];

/// Register a new memory region
pub fn register_region(region: MemRegion) -> Result<(), &'static str> {
    unsafe {
        for slot in REGIONS.iter_mut() {
            if slot.is_none() {
                *slot = Some(region);
                return Ok(());
            }
        }
    }
    Err("Region table full")
}

/// Query region type by address
pub fn region_type_of(addr: PhysAddr) -> Option<RegionType> {
    unsafe {
        for slot in REGIONS.iter() {
            if let Some(region) = slot {
                if region.contains(addr) {
                    return Some(region.region_type);
                }
            }
        }
    }
    None
}

/// Log region map for diagnostics
pub fn print_region_map() {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        unsafe {
            for slot in REGIONS.iter() {
                if let Some(r) = slot {
                    logger.log("[REGION] ");
                    logger.log(&format!(
                        "{:?} @ 0x{:x} - 0x{:x} [{}]",
                        r.region_type,
                        r.start.as_u64(),
                        r.end.as_u64(),
                        r.size_bytes()
                    ));
                }
            }
        }
    }
}
