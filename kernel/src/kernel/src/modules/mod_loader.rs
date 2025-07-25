//! NØNOS Modular Loader – Zero-Trust Secure Runtime
//!
//! Loads `.mod` binaries into RAM with full vault-based identity proofing,
//! ABI version handshake, region sandboxing, capability provisioning, and secure registration.
//! This is the foundation of NØNOS modular kernel model, enabling pluggable, authenticated execution.

use crate::crypto::vault::{is_vault_ready, get_vault_metadata, get_test_key, verify_signature};
use crate::syscall::capabilities::{Capability, CapabilityToken};
use crate::modules::registry::{register_module, ModInstance, ModRuntimeState};
use crate::memory::region::{register_region, RegionType};
use crate::modules::sandbox::trigger_sandbox_violation;
use x86_64::PhysAddr;
use core::time::Duration;

/// ABI Compatibility Level for .mod runtime handshake
const ABI_VERSION: u16 = 1;

/// Result of a module load attempt
#[derive(Debug, Clone)]
pub enum ModuleLoadResult {
    Accepted(CapabilityToken),
    Rejected(&'static str),
    Audited(&'static str),
}

/// Trusted manifest passed during module loading
#[derive(Debug, Clone)]
pub struct ModuleManifest {
    pub name: &'static str,
    pub hash: [u8; 32],
    pub required_caps: &'static [Capability],
    pub version: &'static str,
    pub author: &'static str,
    pub abi_level: u16,
    pub memory_base: PhysAddr,
    pub memory_len: u64,
    pub signature: [u8; 64],
}

/// Entry point for all module loading logic
pub fn load_module(manifest: &ModuleManifest) -> ModuleLoadResult {
    if !is_vault_ready() {
        return fail("Vault not initialized");
    }

    if manifest.name.is_empty() || manifest.required_caps.is_empty() {
        return fail("Invalid manifest structure");
    }

    if manifest.abi_level != ABI_VERSION {
        return fail("ABI mismatch – incompatible module binary");
    }

    if !verify_signature(manifest.hash, manifest.signature) {
        trigger_sandbox_violation(manifest.name, "Invalid cryptographic signature");
        return ModuleLoadResult::Audited("Module signature verification failed");
    }

    // Register memory region for tracking
    register_region(
        manifest.memory_base,
        manifest.memory_len,
        RegionType::ModBinary,
        manifest.name
    );

    let token = CapabilityToken {
        owner_module: manifest.name,
        permissions: manifest.required_caps,
    };

    let instance = ModInstance {
        name: manifest.name,
        token: token.clone(),
        state: ModRuntimeState::Loaded,
        ticks_alive: 0,
        boot_order: 0,
        last_updated: 0,
    };

    match register_module(instance) {
        Ok(_) => ModuleLoadResult::Accepted(token),
        Err(e) => fail(e),
    }
}

/// Dev loader with dummy manifest
pub fn load_core_module(name: &str) -> ModuleLoadResult {
    let dummy = ModuleManifest {
        name,
        hash: [0xAB; 32],
        required_caps: &[Capability::CoreExec, Capability::IO, Capability::IPC],
        version: "0.1.0-alpha",
        author: "coredev@nonos",
        abi_level: ABI_VERSION,
        memory_base: PhysAddr::new(0x800000),
        memory_len: 0x10000,
        signature: [0xAA; 64],
    };

    load_module(&dummy)
}

fn fail(reason: &'static str) -> ModuleLoadResult {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log("[MODLOADER] REJECTED: ");
        logger.log(reason);
    }
    ModuleLoadResult::Rejected(reason)
}
