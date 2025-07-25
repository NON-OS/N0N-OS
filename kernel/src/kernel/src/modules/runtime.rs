//! NØNOS Modular Runtime Lifecycle Engine (Enterprise Edition)
//!
//! Tracks per-module execution state, tick lifetimes, crash diagnostics, boot counters,
//! and restart limits. The runtime registry acts as a volatile execution census within
//! ZeroState and enforces watchdog rules for critical system health.

use crate::syscall::capabilities::CapabilityToken;
use crate::log::logger::try_get_logger;
use alloc::string::String;
use alloc::format;
use spin::RwLock;

/// State representation for a `.mod` binary's current lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModRuntimeState {
    Loaded,
    Running,
    Crashed,
    Halted,
    Terminated,
    Restarting,
    WatchdogTimeout,
}

/// Runtime status block for every active `.mod`
#[derive(Debug, Clone)]
pub struct ModInstance {
    pub name: &'static str,
    pub token: CapabilityToken,
    pub state: ModRuntimeState,
    pub ticks_alive: u64,
    pub boot_order: u8,
    pub last_updated: u64,
    pub crash_count: u8,
    pub restart_attempts: u8,
    pub watchdog_limit: Option<u64>,
}

/// Max `.mod` instances supported in ZeroState runtime
const MAX_MODULES: usize = 64;
static MODULES: RwLock<[Option<ModInstance>; MAX_MODULES]> = RwLock::new([None; MAX_MODULES]);

/// Registers an active `.mod` during loading
pub fn register_module(mut instance: ModInstance) -> Result<(), &'static str> {
    let mut reg = MODULES.write();
    for slot in reg.iter_mut() {
        if slot.is_none() {
            instance.last_updated = 0;
            instance.crash_count = 0;
            instance.restart_attempts = 0;
            *slot = Some(instance);
            return Ok(());
        }
    }
    Err("[runtime] Registry full")
}

/// Increments system tick counter and checks watchdog bounds
pub fn update_ticks() {
    let mut reg = MODULES.write();
    for slot in reg.iter_mut() {
        if let Some(ref mut m) = slot {
            if m.state == ModRuntimeState::Running {
                m.ticks_alive += 1;
                m.last_updated += 1;

                if let Some(limit) = m.watchdog_limit {
                    if m.last_updated > limit {
                        m.state = ModRuntimeState::WatchdogTimeout;
                        audit(&format!("[runtime] {} timed out", m.name));
                    }
                }
            }
        }
    }
}

/// Allows the kernel or scheduler to update runtime state
pub fn set_module_state(name: &str, new_state: ModRuntimeState) {
    let mut reg = MODULES.write();
    for slot in reg.iter_mut() {
        if let Some(ref mut m) = slot {
            if m.name == name {
                if new_state == ModRuntimeState::Crashed {
                    m.crash_count += 1;
                } else if new_state == ModRuntimeState::Restarting {
                    m.restart_attempts += 1;
                }
                m.state = new_state;
                m.last_updated = 0;
                return;
            }
        }
    }
}

/// Returns immutable snapshot of all active modules
pub fn get_all_modules() -> [Option<ModInstance>; MAX_MODULES] {
    MODULES.read().clone()
}

/// Prints runtime census to log output
pub fn print_runtime_snapshot() {
    if let Some(logger) = try_get_logger() {
        let reg = MODULES.read();
        for m in reg.iter().flatten() {
            logger.log("[RUNTIME] ");
            logger.log(m.name);
            logger.log(" | State: ");
            logger.log(match m.state {
                ModRuntimeState::Loaded => "Loaded",
                ModRuntimeState::Running => "Running",
                ModRuntimeState::Crashed => "Crashed",
                ModRuntimeState::Halted => "Halted",
                ModRuntimeState::Terminated => "Terminated",
                ModRuntimeState::Restarting => "Restarting",
                ModRuntimeState::WatchdogTimeout => "WatchdogTimeout",
            });
            logger.log(" | Ticks: ");
            logger.log(&m.ticks_alive.to_string());
        }
    }
}

/// Internal runtime event log
fn audit(msg: &str) {
    if let Some(logger) = try_get_logger() {
        logger.log(msg);
    }
}
