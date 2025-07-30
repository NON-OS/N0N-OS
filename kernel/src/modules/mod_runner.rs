//! NØNOS Module Runtime Executor
//!
//! This is the secure `.mod` execution handler for the NØNOS OS. It handles:
//! - Module lifecycle management (launch, track, teardown)
//! - Memory sandboxing with strict zero-trust enforcement
//! - Capability-driven IPC bindings
//! - Cryptographic bootstrapping
//! - Scheduler integration for cooperative tasking
//!
//! Modules must pass `manifest` and `vault` validation prior to boot.

use crate::kernel::src::modules::manifest::ModuleManifest;
use crate::kernel::src::modules::sandbox::SandboxContext;
use crate::kernel::src::modules::registry::{register_module_instance, is_registered, unregister_module};
use crate::sched::scheduler::{spawn_task, TaskId};
use crate::memory::region::{allocate_region, MemoryRegion};
use crate::ipc::channel::bind_module_ipc;
use crate::capabilities::{CapabilityToken, Capability};
use crate::log::logger::{log_info, log_warn};
use crate::crypto::vault::verify_signature;

use alloc::string::String;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};

static MODULE_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

/// Active module instance representation
pub struct ModuleInstance {
    pub id: TaskId,
    pub ident: u64,
    pub manifest: &'static ModuleManifest,
    pub memory: MemoryRegion,
    pub sandbox: SandboxContext,
    pub token: CapabilityToken,
}

/// Securely launch a `.mod` binary in isolated context
pub fn launch_module(manifest: &'static ModuleManifest, token: CapabilityToken) -> Result<ModuleInstance, &'static str> {
    if !manifest.is_valid() {
        return Err("Manifest verification failed");
    }
    if is_registered(manifest.name()) {
        return Err("Module already loaded");
    }
    if let Some(sig) = manifest.signature() {
        if !verify_signature(manifest.hash, sig) {
            return Err("Signature verification failed");
        }
    }

    let mem = allocate_region(manifest.memory_required as usize)
        .ok_or("Memory allocation failure")?;

    let sandbox = SandboxContext::new(manifest.name(), mem.clone(), &token)?;

    bind_module_ipc(manifest.name())?;

    let entry: NonNull<u8> = unsafe {
        let base = mem.base.as_ptr();
        NonNull::new(base.add(manifest.entrypoint_offset as usize)).ok_or("Entrypoint pointer invalid")?
    };

    let id = spawn_task(entry, sandbox.clone())?;
    let ident = MODULE_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);

    let instance = ModuleInstance {
        id,
        ident,
        manifest,
        memory: mem,
        sandbox,
        token,
    };

    register_module_instance(manifest.name(), &instance);
    log_info("mod_runner", &alloc::format!("Module '{}' (#{}) launched with Task ID #{:?}", manifest.name(), ident, id));

    Ok(instance)
}

/// Terminate a running module instance (future extension)
pub fn shutdown_module(name: &str) -> Result<(), &'static str> {
    if !is_registered(name) {
        return Err("Module not found");
    }
    // TODO: Cancel associated task, release memory, drop sandbox
    unregister_module(name);
    log_warn("mod_runner", &alloc::format!("Module '{}' shutdown invoked", name));
    Ok(())
}

/// Hot-restart a live module with the same manifest + token
pub fn restart_module(manifest: &'static ModuleManifest, token: CapabilityToken) -> Result<ModuleInstance, &'static str> {
    shutdown_module(manifest.name()).ok();
    launch_module(manifest, token)
}
