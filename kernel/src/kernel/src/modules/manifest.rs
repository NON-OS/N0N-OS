//! NØNOS `.mod` Manifest Parser — Extended Edition
//!
//! Responsible for deserializing and verifying signed module metadata from structured formats
//! like CBOR, TOML, or raw memory headers. This parser validates ABI version compatibility,
//! enforces capability schemas, and prepares safe manifest structs for the modular runtime engine.

use crate::syscall::capabilities::Capability;
use crate::modules::mod_loader::ModuleManifest;
use x86_64::PhysAddr;

/// Raw `.mod` manifest extracted from disk, firmware, or boot memory
#[derive(Debug)]
pub struct RawManifest {
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub abi_level: u16,
    pub caps: &'static [&'static str],
    pub hash: [u8; 32],
    pub signature: [u8; 64],
    pub memory_base: u64,
    pub memory_len: u64,
    pub license: Option<&'static str>,
    pub description: Option<&'static str>,
    pub entrypoint: Option<&'static str>,
    pub timestamp: u64,
}

/// Converts a RawManifest into a kernel-usable ModuleManifest
pub fn convert_manifest(raw: &RawManifest) -> Result<ModuleManifest, &'static str> {
    let parsed_caps = parse_caps(raw.caps)?;

    Ok(ModuleManifest {
        name: raw.name,
        version: raw.version,
        author: raw.author,
        abi_level: raw.abi_level,
        hash: raw.hash,
        required_caps: parsed_caps,
        memory_base: PhysAddr::new(raw.memory_base),
        memory_len: raw.memory_len,
        signature: raw.signature,
    })
}

/// Translates raw string capabilities into typed Capability enums
fn parse_caps(raw: &[&str]) -> Result<&'static [Capability], &'static str> {
    const MAX: usize = 16;
    static mut CAPS: [Capability; MAX] = [Capability::None; MAX];
    let mut i = 0;

    for &cap in raw.iter() {
        if i >= MAX {
            return Err("Too many capabilities declared");
        }

        CAPS[i] = match cap.to_ascii_lowercase().as_str() {
            "coreexec" => Capability::CoreExec,
            "io" => Capability::IO,
            "ipc" => Capability::IPC,
            "crypto" => Capability::Crypto,
            "net" => Capability::Network,
            _ => return Err("Unrecognized capability"),
        };
        i += 1;
    }

    unsafe { Ok(&CAPS[..i]) }
}

/// Future implementation for blob-to-struct deserialization
pub fn parse_manifest_from_blob(_blob: &[u8]) -> Result<RawManifest, &'static str> {
    // This will later support embedded CBOR or binary formats in `.mod`
    // Example:
    //   let decoded: RawManifest = cbor::decode(blob)?;
    Err("[parser] Binary manifest parsing not implemented yet")
}
