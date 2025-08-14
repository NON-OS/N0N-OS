//! NØNOS Capability System
//!
//! Zero-trust capability-based access control for all kernel operations.

use alloc::vec::Vec;
use core::fmt;

/// Core capability types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Capability {
    CoreExec = 0x01,    // Core execution rights
    IO = 0x02,          // I/O operations
    SecureMem = 0x03,   // Secure memory access
    CryptoOps = 0x04,   // Cryptographic operations  
    IPC = 0x05,         // Inter-process communication
    Storage = 0x06,     // Storage access
    Network = 0x07,     // Network operations
    ModuleLoad = 0x08,  // Module loading
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Capability token issued to modules
#[derive(Debug, Clone)]
pub struct CapabilityToken {
    pub owner_module: &'static str,
    pub permissions: Vec<Capability>,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
}

impl CapabilityToken {
    /// Create a new token
    pub fn new(owner: &'static str, caps: Vec<Capability>) -> Self {
        Self {
            owner_module: owner,
            permissions: caps,
            issued_at: current_time(),
            expires_at: None,
        }
    }
    
    /// Check if token has a capability
    pub fn has(&self, cap: Capability) -> bool {
        self.permissions.contains(&cap)
    }
    
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            current_time() > expires
        } else {
            false
        }
    }
    
    /// Create a restricted copy with fewer capabilities
    pub fn restrict(&self, allowed: &[Capability]) -> Self {
        let mut restricted_perms = Vec::new();
        for cap in &self.permissions {
            if allowed.contains(cap) {
                restricted_perms.push(*cap);
            }
        }
        
        Self {
            owner_module: self.owner_module,
            permissions: restricted_perms,
            issued_at: current_time(),
            expires_at: self.expires_at,
        }
    }
}

/// Global capability registry
mod registry {
    use super::*;
    use spin::RwLock;
    use alloc::collections::BTreeMap;
    
    static TOKENS: RwLock<BTreeMap<&'static str, CapabilityToken>> = RwLock::new(BTreeMap::new());
    
    pub fn register(token: CapabilityToken) {
        TOKENS.write().insert(token.owner_module, token);
    }
    
    pub fn get(module: &str) -> Option<CapabilityToken> {
        TOKENS.read().get(module).cloned()
    }
    
    pub fn revoke(module: &str) {
        TOKENS.write().remove(module);
    }
}

pub use registry::{register, get, revoke};

/// Initialize capability system
pub fn init_capabilities() {
    // Register kernel capabilities
    register(CapabilityToken::new("kernel", vec![
        Capability::CoreExec,
        Capability::IO,
        Capability::SecureMem,
        Capability::CryptoOps,
        Capability::IPC,
        Capability::Storage,
        Capability::Network,
        Capability::ModuleLoad,
    ]));
    
    log::info!("[CAPS] Capability system initialized");
}

fn current_time() -> u64 {
    unsafe {
        if let Some(ns) = crate::arch::x86_64::time::timer::now_ns_checked() {
            ns / 1_000_000_000 // Convert to seconds
        } else {
            0
        }
    }
}
