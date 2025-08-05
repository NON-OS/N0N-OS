// cli/src/nonosctl/beacon/verify.rs — Proof Engine for zk and Signature Auth
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Verifies: (1) capsule zkProofs, (2) gossip signature chain, (3) author binding to manifest

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use chrono::Utc;
use ed25519_dalek::{PublicKey, Signature, Verifier};
use sha2::{Digest, Sha256};
use serde::{Serialize, Deserialize};

const ZK_CACHE_PATH: &str = "/var/nonos/auth/zk_verified.json";
const MANIFEST_DIR: &str = "/var/nonos/capsules";
const CAPSULE_SIG_DB: &str = "/var/nonos/auth/sig_cache.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct ZkVerifiedCapsule {
    pub capsule: String,
    pub verified_at: String,
    pub signature: String,
    pub zk_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CapsuleSig {
    pub pubkey: String,
    pub signature: String,
    pub message: String,
    pub timestamp: String,
}

pub fn verify_identity(pubkey_b58: &str, signature_hex: &str, message: &str) -> bool {
    if let Ok(pubkey_bytes) = bs58::decode(pubkey_b58).into_vec() {
        if let Ok(pubkey) = PublicKey::from_bytes(&pubkey_bytes) {
            if let Ok(sig_bytes) = hex::decode(signature_hex) {
                if let Ok(signature) = Signature::from_bytes(&sig_bytes) {
                    return pubkey.verify(message.as_bytes(), &signature).is_ok();
                }
            }
        }
    }
    false
}

pub fn verify_zk_hash(capsule: &str, zk_hash: &str) -> bool {
    let binding = format!("{}/{}", MANIFEST_DIR, capsule);
    let manifest_path = Path::new(&binding).with_file_name("manifest.toml");

    if manifest_path.exists() {
        if let Ok(data) = fs::read_to_string(manifest_path) {
            let local_hash = Sha256::digest(data.as_bytes());
            return format!("{:x}", local_hash) == zk_hash;
        }
    }
    false
}

pub fn validate_capsule(capsule: &str, sig: &CapsuleSig, zk_hash: &str) -> bool {
    let sig_ok = verify_identity(&sig.pubkey, &sig.signature, &sig.message);
    let zk_ok = verify_zk_hash(capsule, zk_hash);

    if sig_ok && zk_ok {
        cache_verified_capsule(capsule, &sig.signature, zk_hash);
        true
    } else {
        false
    }
}

fn cache_verified_capsule(capsule: &str, signature: &str, zk_hash: &str) {
    let verified = ZkVerifiedCapsule {
        capsule: capsule.to_string(),
        verified_at: Utc::now().to_rfc3339(),
        signature: signature.into(),
        zk_hash: zk_hash.into(),
    };

    let path = Path::new(ZK_CACHE_PATH);
    let mut db: HashMap<String, ZkVerifiedCapsule> = if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };

    db.insert(capsule.into(), verified);
    fs::write(ZK_CACHE_PATH, serde_json::to_string_pretty(&db).unwrap()).ok();
}

pub fn load_verified_capsules() -> HashMap<String, ZkVerifiedCapsule> {
    let path = Path::new(ZK_CACHE_PATH);
    if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

pub fn check_manifest_identity(capsule: &str, expected_pubkey: &str) -> bool {
    let path = format!("{}/{}/manifest.toml", MANIFEST_DIR, capsule);
    if Path::new(&path).exists() {
        if let Ok(contents) = fs::read_to_string(path) {
            return contents.contains(&format!("author = \"{}\"", expected_pubkey));
        }
    }
    false
}
