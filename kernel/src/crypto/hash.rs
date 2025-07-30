//! NØNOS Cryptographic Hashing Layer – Secure Integrity Engine
//!
//! Supports BLAKE3 hashing for module integrity, state snapshots,
//! memory fingerprints, identity derivation, and secure IPC.
//! All functions are deterministic and memory-safe.

use blake3::Hasher;
use core::fmt::{self, Write};

/// Computes a 32-byte BLAKE3 hash of a given byte slice
pub fn blake3_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(data);
    let hash = hasher.finalize();
    *hash.as_bytes()
}

/// Performs constant-time comparison of two hash outputs
pub fn verify_hash(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b.iter()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Hashes a UTF-8 string into a fixed 32-byte digest
pub fn hash_str(s: &str) -> [u8; 32] {
    blake3_hash(s.as_bytes())
}

/// Debug pretty printer for a BLAKE3 hash
pub fn format_hash(hash: &[u8; 32]) -> HashDisplay {
    HashDisplay(hash)
}

/// Wrapper for hex display of BLAKE3 hash bytes
pub struct HashDisplay(pub &'static [u8; 32]);

impl fmt::Display for HashDisplay {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}
