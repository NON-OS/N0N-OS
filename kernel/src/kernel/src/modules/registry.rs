//! NØNOS Module Registry – Advanced Runtime Engine
//!
//! Tracks all active sandbox modules, their associated capability tokens, runtime lifecycle state,
//! and metrics like uptime, state transitions, and crashes. This registry underpins runtime trust,
//! zero-state auditability, and secure dispatch control. Future extensions include:
//! - Watchdog enforcement
//! - Live memory boundaries
//! - Syscall counters and sandbox telemetry

use crate::syscall::capabilities::CapabilityToken;
use spin::RwLock;
use core::time::Duration;

/// Lifecycle states of a module in RAM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModRuntimeState {
    Loaded,
    Bootstrapped,
    Running,
    Suspended,
    Crashed,
    Terminated,
}

/// Runtime instance for an authenticated module binary
#[derive(Debug, Clone)]
pub struct ModInstance {
    pub name: &'static str,
    pub token: CapabilityToken,
    pub state: ModRuntimeState,
    pub ticks_alive: u64,
    pub boot_order: u8,
    pub last_updated: u64,
}

const MAX_MODULES: usize = 64;

static MODULE_REGISTRY: RwLock<[Option<ModInstance>; MAX_MODULES]> = RwLock::new([None; MAX_MODULES]);

/// Registers a new module into the system runtime table
pub fn register_module(instance: ModInstance) -> Result<(), &'static str> {
    let mut registry = MODULE_REGISTRY.write();
    for (i, slot) in registry.iter_mut().enumerate() {
        if slot.is_none() {
            let mut instance = instance;
            instance.boot_order = i as u8;
            instance.last_updated = 0;
            *slot = Some(instance);
            return Ok(());
        }
    }
    Err("[REGISTRY] Module table full")
}

/// Returns all registered modules
pub fn list_modules() -> Vec<ModInstance> {
    let registry = MODULE_REGISTRY.read();
    registry.iter().filter_map(|m| m.clone()).collect()
}

/// Find a module by name
pub fn find_module(name: &str) -> Option<ModInstance> {
    let registry = MODULE_REGISTRY.read();
    for instance in registry.iter() {
        if let Some(inst) = instance {
            if inst.name == name {
                return Some(inst.clone());
            }
        }
    }
    None
}

/// Mutate the runtime state of a module
pub fn update_module_state(name: &str, new_state: ModRuntimeState, tick: u64) -> Result<(), &'static str> {
    let mut registry = MODULE_REGISTRY.write();
    for slot in registry.iter_mut() {
        if let Some(ref mut inst) = slot {
            if inst.name == name {
                inst.state = new_state;
                inst.last_updated = tick;
                return Ok(());
            }
        }
    }
    Err("[REGISTRY] Module not found")
}

/// Update tick-based metrics (used by scheduler/watchdog)
pub fn increment_uptime_ticks() {
    let mut registry = MODULE_REGISTRY.write();
    for slot in registry.iter_mut() {
        if let Some(ref mut inst) = slot {
            if matches!(inst.state, ModRuntimeState::Running) {
                inst.ticks_alive += 1;
            }
        }
    }
}

/// Print a runtime status summary
pub fn print_registry_snapshot() {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        let snapshot = list_modules();
        logger.log("[MODS] Live Module Snapshot:");
        for m in snapshot {
            logger.log(&format!(
                "{} | {:?} | {} ticks | boot#{}",
                m.name, m.state, m.ticks_alive, m.boot_order
            ));
        }
    }
}
