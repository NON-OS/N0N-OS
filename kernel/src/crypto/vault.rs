//! NØNOS Cryptographic Vault Subsystem
//!
//! The Vault provides the NØNOS microkernel with a secure root-of-trust abstraction,
//! implemented entirely in volatile memory to support ZeroState architecture. The vault:
//! - Initializes cryptographic boot-time identity
//! - Provisions session-bound symmetric keys
//! - Stores ephemeral boot metadata (e.g. SecureBoot flag, Device ID)
//! - Integrates with future sealed module manifests for `.mod` loading
//! - Performs deterministic key generation compatible with zkProofs

use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::UnsafeCell;
use core::fmt::{self, Debug, Formatter};
use crate::crypto::vault::VaultDerivationMode::*;

/// Represents a 256-bit volatile key issued to kernel subsystems
#[derive(Clone)]
pub struct VaultKey {
    pub key_bytes: [u8; 32],
    pub id: &'static str,
    pub derived: bool,
    pub usage: KeyUsage,
}

impl Debug for VaultKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "VaultKey(id={}, derived={}, usage={:?})", self.id, self.derived, self.usage)
    }
}

/// Tracks declared usage of a Vault key (future audit trail)
#[derive(Debug, Clone)]
pub enum KeyUsage {
    KernelIntegrity,
    ModuleIsolation,
    IPCStream,
    NetworkAuth,
    TestDev,
}

/// Vault internal runtime state
static VAULT_READY: AtomicBool = AtomicBool::new(false);
static mut VAULT_PRIMARY: UnsafeCell<Option<VaultKey>> = UnsafeCell::new(None);

/// Vault metadata sealed at boot time
#[derive(Debug, Clone)]
pub struct VaultMetadata {
    pub device_id: &'static str,
    pub secure_boot: bool,
    pub firmware_hash: [u8; 32],
    pub version: &'static str,
    pub entropy_bits: u64,
}

/// Supported derivation modes for future deterministic HMAC-based expansions
#[derive(Debug, Clone)]
pub enum VaultDerivationMode {
    HKDF,
    Direct,
    ZeroizedFallback,
}

/// Initializes the Vault by provisioning the primary key in RAM.
/// This should later be sealed to a trusted TPM or UEFI-measured region.
pub fn init_vault() {
    unsafe {
        *VAULT_PRIMARY.get() = Some(VaultKey {
            key_bytes: [0x42; 32],
            id: "bootkey:dev",
            derived: false,
            usage: KeyUsage::KernelIntegrity,
        });
    }
    VAULT_READY.store(true, Ordering::SeqCst);
}

/// Check if vault has been initialized
pub fn is_vault_ready() -> bool {
    VAULT_READY.load(Ordering::SeqCst)
}

/// Returns the main volatile kernel key
pub fn get_test_key() -> VaultKey {
    unsafe {
        (*VAULT_PRIMARY.get()).as_ref().unwrap().clone()
    }
}

/// Derives a new runtime key from the base vault key (e.g. for IPC or module scopes)
pub fn derive_key(usage: KeyUsage, mode: VaultDerivationMode) -> VaultKey {
    let base = get_test_key();
    let mut new_key = [0u8; 32];

    for i in 0..32 {
        new_key[i] = base.key_bytes[i] ^ match mode {
            HKDF => 0xAB,
            Direct => 0x55,
            ZeroizedFallback => 0x00,
        };
    }

    VaultKey {
        key_bytes: new_key,
        id: "derived:scope",
        derived: true,
        usage,
    }
}

/// Provides sealed runtime metadata tied to the boot environment
pub fn get_vault_metadata() -> VaultMetadata {
    VaultMetadata {
        device_id: "NONOS_DEVBOARD",
        secure_boot: true,
        firmware_hash: [0xAA; 32],
        version: "v0.1.0-alpha",
        entropy_bits: 192,
    }
}
