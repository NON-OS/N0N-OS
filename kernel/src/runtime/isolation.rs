//! NØNOS Isolation Boundary Enforcement
//!
//! Implements security perimeters and isolation mechanisms for capsules.

use core::sync::atomic::{AtomicU64, Ordering};
use spin::RwLock;

use crate::capabilities::{Capability, CapabilityToken};
use crate::memory::virt::VmFlags;

/// Security perimeter for a capsule
#[derive(Debug, Clone)]
pub struct SecurityPerimeter {
    pub memory_bounds: (u64, u64),
    pub allowed_syscalls: Vec<u64>,
    pub ipc_whitelist: Vec<String>,
    pub max_cpu_percent: u8,
    pub max_memory_mb: usize,
}

impl SecurityPerimeter {
    pub fn from_capabilities(token: &CapabilityToken) -> Self {
        let mut allowed_syscalls = Vec::new();
        
        // Map capabilities to allowed syscalls
        for cap in &token.permissions {
            match cap {
                Capability::CoreExec => {
                    allowed_syscalls.extend(&[0x02, 0x04]); // GetTime, ModSpawn
                }
                Capability::IO => {
                    allowed_syscalls.push(0x01); // Log
                }
                Capability::Crypto => {
                    allowed_syscalls.push(0x05); // ReadEntropy
                }
                Capability::IPC => {
                    allowed_syscalls.extend(&[0x06, 0x07]); // IPCSend, IPCReceive
                }
                _ => {}
            }
        }
        
        Self {
            memory_bounds: (0, 0),
            allowed_syscalls,
            ipc_whitelist: Vec::new(),
            max_cpu_percent: 25,
            max_memory_mb: 64,
        }
    }
    
    /// Check if a syscall is allowed
    pub fn can_syscall(&self, syscall_id: u64) -> bool {
        self.allowed_syscalls.contains(&syscall_id)
    }
    
    /// Check if IPC to target is allowed
    pub fn can_ipc_to(&self, target: &str) -> bool {
        self.ipc_whitelist.is_empty() || self.ipc_whitelist.contains(&target.to_string())
    }
    
    /// Check if memory access is within bounds
    pub fn check_memory_access(&self, addr: u64, size: usize) -> bool {
        let end = addr + size as u64;
        addr >= self.memory_bounds.0 && end <= self.memory_bounds.1
    }
}

/// Isolation boundary between capsules
#[derive(Debug)]
pub struct IsolationBoundary {
    pub id: u64,
    pub perimeter: SecurityPerimeter,
    pub violations: AtomicU64,
}

impl IsolationBoundary {
    pub fn new(perimeter: SecurityPerimeter) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
            perimeter,
            violations: AtomicU64::new(0),
        }
    }
    
    /// Record a violation
    pub fn record_violation(&self, reason: &str) {
        self.violations.fetch_add(1, Ordering::SeqCst);
        log::warn!("[ISOLATION] Boundary {} violation: {}", self.id, reason);
    }
    
    /// Check and enforce boundary
    pub fn enforce(&self, check: impl FnOnce(&SecurityPerimeter) -> bool) -> bool {
        if !check(&self.perimeter) {
            self.record_violation("Check failed");
            false
        } else {
            true
        }
    }
}

// Global boundary registry
static BOUNDARIES: RwLock<heapless::FnvIndexMap<u64, IsolationBoundary, 256>> = 
    RwLock::new(heapless::FnvIndexMap::new());

/// Initialize isolation subsystem
pub fn init_boundaries() {
    log::info!("[ISOLATION] Boundary enforcement initialized");
}

/// Create a new isolation boundary
pub fn create_boundary(perimeter: SecurityPerimeter) -> u64 {
    let boundary = IsolationBoundary::new(perimeter);
    let id = boundary.id;
    
    BOUNDARIES.write().insert(id, boundary).ok();
    
    log::info!("[ISOLATION] Created boundary {}", id);
    id
}

/// Get boundary by ID
pub fn get_boundary(id: u64) -> Option<IsolationBoundary> {
    BOUNDARIES.read().get(&id).cloned()
}

/// Remove boundary
pub fn remove_boundary(id: u64) {
    BOUNDARIES.write().remove(&id);
    log::info!("[ISOLATION] Removed boundary {}", id);
}

/// Check cross-boundary communication
pub fn check_cross_boundary(from_id: u64, to_id: u64, message: &[u8]) -> bool {
    let boundaries = BOUNDARIES.read();
    
    if let (Some(from), Some(to)) = (boundaries.get(&from_id), boundaries.get(&to_id)) {
        // Check if communication is allowed
        // This is a simplified check - real implementation would be more complex
        
        if message.len() > 65536 {
            from.record_violation("Message too large");
            return false;
        }
        
        true
    } else {
        false
    }
}

/// Memory protection setup for isolation
pub fn setup_memory_protection(boundary_id: u64, base: u64, size: usize) -> Result<(), &'static str> {
    let mut boundaries = BOUNDARIES.write();
    
    if let Some(boundary) = boundaries.get_mut(&boundary_id) {
        boundary.perimeter.memory_bounds = (base, base + size as u64);
        
        // Set up guard pages
        use crate::memory::virt;
        use x86_64::{VirtAddr, PhysAddr};
        
        // Guard page before
        let guard_before = VirtAddr::new(base - 4096);
        let _ = virt::protect4k(guard_before, VmFlags::empty()); // No permissions
        
        // Guard page after
        let guard_after = VirtAddr::new(base + size as u64);
        let _ = virt::protect4k(guard_after, VmFlags::empty());
        
        log::info!("[ISOLATION] Memory protection set for boundary {}: 0x{:x}-0x{:x}", 
                   boundary_id, base, base + size as u64);
        
        Ok(())
    } else {
        Err("Boundary not found")
    }
}
