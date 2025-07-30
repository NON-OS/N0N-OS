//! NØNOS Modular Runtime: Manifest ABI Definition
//!
//! This manifest defines the binary interface between sandboxed `.mod` modules
//! and the NØNOS runtime kernel. It encodes trust-scoped metadata including capability
//! declarations, cryptographic fingerprints, execution entrypoints, and ABI layout.
//!
//! Manifest headers are validated at load-time by the `mod_loader.rs`, and further enforced
//! by the `sandbox.rs`, `registry.rs`, and `mod_runner.rs` subsystems. This layout must remain
//! forward-compatible across kernel versions.

use crate::capabilities::Capability;
use core::fmt;

/// 4-byte magic header to identify valid `.mod` manifests
pub const MANIFEST_MAGIC: [u8; 4] = *b"MODX";

/// Current ABI version for the manifest structure
pub const MANIFEST_VERSION: u16 = 1;

/// Maximum number of UTF-8 bytes permitted in `module_name`
pub const MODULE_NAME_MAX: usize = 32;

/// NØNOS Module Manifest Header
/// 
/// This header precedes any loadable `.mod` binary. It must be aligned, verified,
/// and cryptographically validated before any runtime acceptance.
#[repr(C, packed)]
#[derive(Clone)]
pub struct ModuleManifest {
    pub magic: [u8; 4],                    // Magic identifier: "MODX"
    pub format_version: u16,              // ABI compatibility layer
    pub module_name: [u8; MODULE_NAME_MAX], // UTF-8, null-terminated
    pub version_code: u32,                // Encoded as (major << 16 | minor << 8 | patch)
    pub hash: [u8; 32],                   // SHA-256 fingerprint of payload
    pub entrypoint_offset: u64,           // Executable byte offset
    pub memory_required: u64,             // Requested runtime heap space in bytes
    pub stack_size: u64,                  // Suggested stack allocation size
    pub num_caps: u16,                    // Capability count
    pub caps_ptr: *const Capability,      // Raw pointer to capability array
    pub signature_ptr: *const u8,         // Optional cryptographic signature
    pub signature_len: u16,               // Signature length in bytes
    pub reserved: [u8; 4],                // Alignment / reserved future fields
}

unsafe impl Send for ModuleManifest {}
unsafe impl Sync for ModuleManifest {}

impl ModuleManifest {
    /// Performs lightweight header validation
    pub fn is_valid(&self) -> bool {
        self.magic == MANIFEST_MAGIC &&
        self.format_version == MANIFEST_VERSION &&
        self.num_caps as usize <= Capability::MAX_DECLARED
    }

    /// Returns module name as string slice
    pub fn name(&self) -> &str {
        let nul_pos = self.module_name.iter().position(|&c| c == 0).unwrap_or(MODULE_NAME_MAX);
        core::str::from_utf8(&self.module_name[..nul_pos]).unwrap_or("<invalid>")
    }

    /// Fetch declared capabilities slice
    pub fn declared_capabilities(&self) -> &[Capability] {
        unsafe { core::slice::from_raw_parts(self.caps_ptr, self.num_caps as usize) }
    }

    /// Return signature slice if present
    pub fn signature(&self) -> Option<&[u8]> {
        if self.signature_len > 0 {
            unsafe { Some(core::slice::from_raw_parts(self.signature_ptr, self.signature_len as usize)) }
        } else {
            None
        }
    }

    /// Decode semantic version from packed integer
    pub fn version(&self) -> (u16, u8, u8) {
        let major = (self.version_code >> 16) as u16;
        let minor = ((self.version_code >> 8) & 0xFF) as u8;
        let patch = (self.version_code & 0xFF) as u8;
        (major, minor, patch)
    }

    /// Return formatted version string
    pub fn version_str(&self) -> alloc::string::String {
        let (maj, min, patch) = self.version();
        alloc::format!("{}.{}.{}", maj, min, patch)
    }
}

impl fmt::Debug for ModuleManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleManifest")
            .field("name", &self.name())
            .field("version", &self.version_str())
            .field("entrypoint_offset", &self.entrypoint_offset)
            .field("memory_required", &self.memory_required)
            .field("stack_size", &self.stack_size)
            .field("capabilities", &self.declared_capabilities())
            .field("signature_len", &self.signature_len)
            .finish()
    }
}

/// ABI-Level Constants
pub mod abi_consts {
    pub const ALIGNMENT: usize = 64;
    pub const HEADER_SIZE: usize = 128;
    pub const SIGNATURE_MAX: usize = 512;
}
