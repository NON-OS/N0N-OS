//! capsule.rs — NØNOS Boot Capsule Handler -eK chadding the Next Gen OS ooof 
//!
//! Defines the structure, parsing, and verification of a sealed execution capsule
//! used to securely deliver and launch ZeroState payloads (kernel, modules, etc.)
//! Each capsule contains cryptographic metadata and optional zk-SNARK attestations.

use crate::verify::{verify_capsule, CapsuleVerification, CapsuleMetadata};
use crate::log::logger::{log_info, log_warn};
use sha2::{Digest, Sha256};

/// Capsule constants for magic header signature and versioning.
pub const CAPSULE_MAGIC: &[u8; 4] = b"N0N\0";
pub const CAPSULE_VERSION: u8 = 1;

/// CapsuleHeader — Structure defining the sealed blob layout.
/// This metadata must be present at the start of the capsule memory.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct CapsuleHeader {
    pub magic: [u8; 4],        // Format signature, must be b"N0N\0"
    pub version: u8,          // Format version
    pub flags: u8,            // Bitflags (ZK-required, Compressed, etc.)
    pub offset_sig: u32,      // Byte offset to signature or proof
    pub offset_payload: u32,  // Byte offset to capsule payload
    pub len_sig: u32,         // Length of proof/signature
    pub len_payload: u32,     // Length of payload
}

/// In-memory view of a sealed capsule.
pub struct Capsule {
    pub header: CapsuleHeader,
    pub blob: &'static [u8],
}

impl CapsuleHeader {
    /// Validate capsule header integrity.
    pub fn is_valid(&self) -> bool {
        &self.magic == CAPSULE_MAGIC && self.version == CAPSULE_VERSION
    }

    /// Calculate SHA-256 of the payload region (used for commitments).
    pub fn commitment(&self, blob: &[u8]) -> Option<[u8; 32]> {
        let start = self.offset_payload as usize;
        let end = start + self.len_payload as usize;
        if end > blob.len() {
            return None;
        }
        Some(Sha256::digest(&blob[start..end]).into())
    }
}

impl Capsule {
    /// Construct a Capsule from raw memory.
    pub fn from_blob(blob: &'static [u8]) -> Result<Self, &'static str> {
        if blob.len() < core::mem::size_of::<CapsuleHeader>() {
            return Err("Blob too small for capsule header");
        }
        let header = unsafe { &*(blob.as_ptr() as *const CapsuleHeader) };
        if !header.is_valid() {
            return Err("Invalid capsule magic or version");
        }
        Ok(Capsule {
            header: *header,
            blob,
        })
    }

    /// Extracts internal metadata for verification engine
    pub fn to_metadata(&self) -> CapsuleMetadata {
        CapsuleMetadata {
            version: self.header.version,
            flags: self.header.flags,
            offset_sig: self.header.offset_sig as usize,
            offset_payload: self.header.offset_payload as usize,
            len_sig: self.header.len_sig as usize,
            len_payload: self.header.len_payload as usize,
        }
    }

    /// Run the full verification pipeline on the capsule.
    pub fn verify(&self) -> CapsuleVerification {
        let meta = self.to_metadata();
        verify_capsule(self.blob, &meta)
    }

    /// Launch capsule payload securely.
    /// In the future: map payload into ZeroState, validate memory boundary, transfer control.
    pub fn launch(self) -> Result<(), &'static str> {
        match self.verify() {
            CapsuleVerification::StaticVerified | CapsuleVerification::ZkVerified => {
                let start = self.header.offset_payload as usize;
                let end = start + self.header.len_payload as usize;
                let payload = &self.blob[start..end];
                log_info("capsule", &format!("Payload verified ({} bytes). Launching capsule.", payload.len()));
                // TODO: Memory mapping + transition to exec
                Ok(())
            }
            CapsuleVerification::Failed(e) => {
                log_warn("capsule", e);
                Err("Capsule verification failed")
            }
        }
    }
}

