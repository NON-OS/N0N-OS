//! NØNOS Signature Verification Interface – Production-Grade
//!
//! Cryptographically validates `.mod` manifests and ZeroState attestations using
//! Ed25519 by default, with planned support for ECDSA and other curves. This layer
//! ensures that all boot artifacts are cryptographically authorized.

use ed25519_dalek::{Verifier, PublicKey, Signature};
use ed25519_dalek::ed25519::signature::Signature as _;
use sha3::{Digest, Sha3_256};

/// Supported signature verification schemes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigAlgo {
    Ed25519,
    EcdsaP256, // Placeholder for future implementation
    Unsupported,
}

/// A structured signature proof for manifest verification
#[derive(Debug)]
pub struct SignatureBlock {
    pub algo: SigAlgo,
    pub pubkey: [u8; 32],
    pub signature: [u8; 64],
    pub payload_digest: [u8; 32],
    pub signer: &'static str,
}

/// High-level manifest verification entrypoint
pub fn validate_signature_block(block: &SignatureBlock, payload: &[u8]) -> bool {
    let digest = sha3_digest(payload);
    if block.payload_digest != digest {
        audit("[sig] digest mismatch on payload");
        return false;
    }

    match block.algo {
        SigAlgo::Ed25519 => {
            let valid = verify_ed25519_signature(&block.pubkey, payload, &block.signature);
            if valid {
                audit(&format!("[sig] Ed25519 verified: {}", block.signer));
            } else {
                audit(&format!("[sig] Ed25519 INVALID: {}", block.signer));
            }
            valid
        },
        SigAlgo::Unsupported | SigAlgo::EcdsaP256 => {
            audit("[sig] unsupported signature scheme");
            false
        },
    }
}

/// Verifies Ed25519 signature against message
pub fn verify_ed25519_signature(
    public_key_bytes: &[u8; 32],
    message: &[u8],
    signature_bytes: &[u8; 64],
) -> bool {
    let pk = match PublicKey::from_bytes(public_key_bytes) {
        Ok(key) => key,
        Err(_) => return false,
    };

    let sig = match Signature::from_bytes(signature_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    pk.verify(message, &sig).is_ok()
}

/// Computes SHA3-256 digest of a message
pub fn sha3_digest(msg: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(msg);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Log sink for audit and runtime tracing
fn audit(msg: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log(msg);
    }
}
