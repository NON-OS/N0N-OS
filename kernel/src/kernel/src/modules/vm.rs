//! NØNOS Per-Module Virtual Memory Isolation Layer
//!
//! Provides dynamic virtual memory scoping for each sandboxed `.mod` binary.
//! Implements runtime-enforced fencing, per-mod virtual ranges, optional NX enforcement,
//! and deferred support for per-mod heap allocators.

use x86_64::VirtAddr;
use core::ops::Range;
use spin::RwLock;
use alloc::format;
use crate::log::logger::try_get_logger;

/// Maximum number of concurrent modules with isolated memory
const MAX_MODS: usize = 64;

/// Virtual memory region allocated to a module
#[derive(Debug, Clone)]
pub struct ModVMRegion {
    pub name: &'static str,
    pub virt_range: Range<u64>,
    pub active: bool,
    pub enforce_nx: bool,
    pub dirty: bool,
    pub fault_count: u32,
}

/// Global runtime table of all module VM assignments
static VM_REGIONS: RwLock<[Option<ModVMRegion>; MAX_MODS]> = RwLock::new([None; MAX_MODS]);

/// Assigns a protected VM range to a module on load
pub fn assign_region(name: &'static str, base: u64, len: u64, enforce_nx: bool) -> Result<(), &'static str> {
    let mut reg = VM_REGIONS.write();

    for entry in reg.iter_mut() {
        if entry.is_none() {
            *entry = Some(ModVMRegion {
                name,
                virt_range: base..(base + len),
                active: true,
                enforce_nx,
                dirty: false,
                fault_count: 0,
            });
            audit(&format!("[vm] region assigned to {name}"));
            return Ok(());
        }
    }

    Err("[vm] VM region table full")
}

/// Validates a memory access by a module at runtime
pub fn validate_access(module: &str, addr: VirtAddr, exec: bool) -> bool {
    let mut reg = VM_REGIONS.write();
    for entry in reg.iter_mut().flatten() {
        if entry.name == module && entry.active {
            let a = addr.as_u64();
            let valid = entry.virt_range.contains(&a);

            if entry.enforce_nx && exec {
                entry.fault_count += 1;
                audit(&format!("[vm] NX violation by {}", module));
                return false;
            }

            if !valid {
                entry.fault_count += 1;
                audit(&format!("[vm] OOB memory access by {}", module));
            }

            return valid;
        }
    }
    false
}

/// Deactivates a region after termination or sandbox kill
pub fn clear_region(module: &str) {
    let mut reg = VM_REGIONS.write();
    for entry in reg.iter_mut() {
        if let Some(ref mut region) = entry {
            if region.name == module {
                region.active = false;
                region.dirty = true;
                audit(&format!("[vm] region cleared for {}", module));
            }
        }
    }
}

/// Provides diagnostic inspection of all active VM regions
pub fn print_vm_map() {
    if let Some(logger) = try_get_logger() {
        let reg = VM_REGIONS.read();
        for entry in reg.iter().flatten() {
            logger.log("[VM] ");
            logger.log(entry.name);
            logger.log(" | range: ");
            logger.log(&format!("0x{:x}..0x{:x}", entry.virt_range.start, entry.virt_range.end));
            logger.log(" | NX: ");
            logger.log(if entry.enforce_nx { "ENABLED" } else { "DISABLED" });
            logger.log(" | faults: ");
            logger.log(&entry.fault_count.to_string());
        }
    }
}

/// Logs audit events related to VM protection
fn audit(msg: &str) {
    if let Some(logger) = try_get_logger() {
        logger.log(msg);
    }
}
