// cli/src/nonosctl/depin.rs — NØN-OS DePIN: proof + fees + mesh broadcast
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies

use chrono::Utc;
use ed25519_dalek::{Keypair, Signature, Signer, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs, io::{Read, Write},
    net::UdpSocket,
    path::Path,
    time::{Duration, SystemTime},
};

const KEY_DIR: &str = "/var/nonos/keys";
const NODE_KEY: &str = "/var/nonos/keys/node.ed25519";
const LEDGER_PATH: &str = "/var/nonos/ledger.json";
const PROOF_OUTBOX: &str = "/var/nonos/mesh/outbox"; // handoff to mesh daemon
const HOSTNAME_PATH: &str = "/etc/hostname";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfInfra {
    pub node_id: String,           // b58 of public key
    pub hostname: String,
    pub ts: String,
    pub cpu_logical: u16,
    pub mem_total_mb: u64,
    pub mem_free_mb: u64,
    pub disk_free_mb: u64,
    pub net_up_ms: u128,
    pub addr_hint: Option<String>, // non-PII, optional local mesh addr/port
    pub runtime_hash: String,      // beacon/runtime state hash if available
    pub sig: String,               // hex(ed25519)
    pub ver: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeeLedger {
    pub version: String,
    pub total_installs: u64,
    pub total_fees: f64,
    pub entries: Vec<FeeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEntry {
    pub ts: String,
    pub module: String,
    pub fee_nonos: f64,
    pub capsule: String,
    pub tx_hash: Option<String>, // optional L2 anchor later
}

pub fn init_keys() -> Result<Keypair, String> {
    fs::create_dir_all(KEY_DIR).map_err(err)?;
    if Path::new(NODE_KEY).exists() {
        let bytes = fs::read(NODE_KEY).map_err(err)?;
        if bytes.len() != SECRET_KEY_LENGTH + PUBLIC_KEY_LENGTH {
            return Err("invalid key length".into());
        }
        let secret = ed25519_dalek::SecretKey::from_bytes(&bytes[0..SECRET_KEY_LENGTH])
            .map_err(|e| e.to_string())?;
        let public = ed25519_dalek::PublicKey::from_bytes(&bytes[SECRET_KEY_LENGTH..])
            .map_err(|e| e.to_string())?;
        Ok(Keypair { secret, public })
    } else {
        let kp = Keypair::generate(&mut OsRng);
        let mut buf = Vec::with_capacity(SECRET_KEY_LENGTH + PUBLIC_KEY_LENGTH);
        buf.extend_from_slice(kp.secret.as_bytes());
        buf.extend_from_slice(kp.public.as_bytes());
        fs::write(NODE_KEY, &buf).map_err(err)?;
        Ok(kp)
    }
}

pub fn build_and_sign_proof(runtime_hash: Option<String>) -> Result<ProofOfInfra, String> {
    let kp = init_keys()?;
    let hostname = fs::read_to_string(HOSTNAME_PATH).unwrap_or_else(|_| "nonos-node".into());
    let cpu_logical = num_cpus::get() as u16;

    // mem info
    let (mem_total_mb, mem_free_mb) = mem_info_mb();

    // disk free (root)
    let disk_free_mb = {
        let stat = fs2::free_space("/").map_err(err)?;
        stat / (1024 * 1024)
    };

    // uptime as network hint (ms)
    let net_up_ms = uptime_ms();

    // best effort UDP addr hint (no PII—just a socket local addr)
    let addr_hint = UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|s| s.local_addr().ok())
        .map(|a| a.to_string());

    let base = serde_json::json!({
        "node_id": b58(&kp.public.to_bytes()),
        "hostname": hostname.trim(),
        "ts": Utc::now().to_rfc3339(),
        "cpu_logical": cpu_logical,
        "mem_total_mb": mem_total_mb,
        "mem_free_mb":  mem_free_mb,
        "disk_free_mb": disk_free_mb,
        "net_up_ms":    net_up_ms,
        "addr_hint":    addr_hint,
        "runtime_hash": runtime_hash.clone().unwrap_or_default(),
        "ver": "depIn-v1"
    });

    let mut hasher = Sha256::new();
    hasher.update(base.to_string().as_bytes());
    let digest = hasher.finalize();

    let sig: Signature = kp.sign(&digest);

    let proof = ProofOfInfra {
        node_id: b58(&kp.public.to_bytes()),
        hostname: hostname.trim().into(),
        ts: Utc::now().to_rfc3339(),
        cpu_logical,
        mem_total_mb,
        mem_free_mb,
        disk_free_mb,
        net_up_ms,
        addr_hint,
        runtime_hash: runtime_hash.unwrap_or_default(),
        sig: hex::encode(sig.to_bytes()),
        ver: "depIn-v1",
    };

    // drop to outbox for the mesh daemon to gossip (decoupled from CLI)
    fs::create_dir_all(PROOF_OUTBOX).ok();
    let fpath = format!("{}/proof_{}.json", PROOF_OUTBOX, proof.ts.replace(':', "-"));
    fs::write(&fpath, serde_json::to_vec_pretty(&proof).map_err(err)?).map_err(err)?;

    Ok(proof)
}

/// Record a micro-fee for a `.mod` action (install/execute)
pub fn record_fee(module: &str, fee_nonos: f64, capsule: &str, tx_hash: Option<String>) -> Result<(), String> {
    fs::create_dir_all("/var/nonos").ok();
    let mut ledger: FeeLedger = if Path::new(LEDGER_PATH).exists() {
        let s = fs::read_to_string(LEDGER_PATH).map_err(err)?;
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        FeeLedger { version: "v1".into(), ..Default::default() }
    };

    ledger.total_installs += 1;
    ledger.total_fees += fee_nonos;
    ledger.entries.push(FeeEntry {
        ts: Utc::now().to_rfc3339(),
        module: module.into(),
        fee_nonos,
        capsule: capsule.into(),
        tx_hash,
    });

    fs::write(LEDGER_PATH, serde_json::to_vec_pretty(&ledger).map_err(err)?).map_err(err)?;
    Ok(())
}

/// Helper: human-readable base58
fn b58(bytes: &[u8]) -> String { bs58::encode(bytes).into_string() }

fn err<E: std::fmt::Display>(e: E) -> String { e.to_string() }

fn mem_info_mb() -> (u64, u64) {
    // Linux /proc/meminfo
    if let Ok(s) = fs::read_to_string("/proc/meminfo") {
        let mut total = 0_u64;
        let mut free  = 0_u64;
        for line in s.lines() {
            if line.starts_with("MemTotal:") {
                total = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) / 1024;
            }
            if line.starts_with("MemAvailable:") || line.starts_with("MemFree:") {
                let v = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) / 1024;
                if v > free { free = v; }
            }
        }
        return (total, free);
    }
    (0, 0)
}

fn uptime_ms() -> u128 {
    // Prefer /proc/uptime
    if let Ok(s) = fs::read_to_string("/proc/uptime") {
        if let Some(first) = s.split_whitespace().next() {
            if let Ok(f) = first.parse::<f64>() {
                return (f * 1000.0) as u128;
            }
        }
    }
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or(Duration::ZERO).as_millis()
}

// ----- Hooks you already have / can wire:
//  - Mesh daemon reads PROOF_OUTBOX/*.json and gossips to peers
//  - Beacon verifies sig + runtime_hash and updates trust score
//  - .mod installer calls `record_fee()` after successful verify
