//! NØNOS Module Admission Authority
//!
//! Verifies `.mod` manifests using a decentralized trust policy.
//! This integrates zk-proof of signer identity, root key rotation,
//! DAO-curated signer registry, and fine-grained capability scoping.

use crate::crypto::vault::{verify_signature, get_root_pubkeys, verify_zk_attestation};
use crate::modules::manifest::ModuleManifest;
use crate::capabilities::{CapabilityToken, Capability};
use crate::log::logger::{log_info, log_warn};

use alloc::vec::Vec;
use alloc::collections::BTreeSet;

/// Result of decentralized manifest verification
pub enum AuthResult {
    Verified(CapabilityToken),
    Rejected(&'static str),
}

/// DAO-governed trusted signer registry (RAM-loaded)
static mut TRUSTED_SIGNERS: Option<BTreeSet<[u8; 32]>> = None;

/// Load the trusted root registry from vault
pub fn init_trusted_signers() {
    let keys = get_root_pubkeys();
    unsafe {
        TRUSTED_SIGNERS = Some(BTreeSet::from_iter(keys));
    }
    log_info("auth", "Trusted signer root initialized");
}

/// Add a DAO-approved signer (zk-proven identity)
pub fn approve_signer(pubkey: [u8; 32]) {
    unsafe {
        if let Some(registry) = TRUSTED_SIGNERS.as_mut() {
            registry.insert(pubkey);
        }
    }
    log_info("auth", &format!("Signer approved: {:x?}", &pubkey[..4]));
}

/// Core manifest authentication and scope filtering
pub fn authenticate_manifest(manifest: &ModuleManifest) -> AuthResult {
    let sig = manifest.signature.ok_or("Missing signature").unwrap();
    let zk = manifest.zk_proof;

    // Derive attested identity from zk proof
    let signer_id = match zk {
        Some(proof) => {
            match verify_zk_attestation(proof) {
                Some(id) => id,
                None => return AuthResult::Rejected("zkProof identity invalid"),
            }
        }
        None => manifest.signer_id,
    };

    if signer_id.is_none() {
        return AuthResult::Rejected("No valid signer identity");
    }

    let signer_key = signer_id.unwrap();

    // Validate against DAO signer registry
    let trusted = unsafe {
        TRUSTED_SIGNERS
            .as_ref()
            .map(|set| set.contains(&signer_key))
            .unwrap_or(false)
    };

    if !trusted {
        return AuthResult::Rejected("Signer not in trusted DAO registry");
    }

    if !verify_signature(manifest.hash, sig, &signer_key) {
        return AuthResult::Rejected("Signature mismatch");
    }

    // Issue scoped capabilities (future: filter by role or NFT)
    let token = CapabilityToken {
        owner_module: manifest.name,
        permissions: manifest.required_caps,
    };

    log_info("auth", &format!(
        "Authenticated module '{}' by {:x?}, caps = {}",
        manifest.name, &signer_key[..4], token.permissions.len()
    ));

    AuthResult::Verified(token)
}

/// DAO-unsafe fallback (for local devnet only)
pub fn unsafe_allow_all_caps(name: &'static str) -> CapabilityToken {
    CapabilityToken {
        owner_module: name,
        permissions: &[
            Capability::CoreExec,
            Capability::IO,
            Capability::IPC,
            Capability::Crypto,
            Capability::Storage,
        ],
    }
}
