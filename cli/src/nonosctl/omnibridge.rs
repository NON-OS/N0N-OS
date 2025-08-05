// cli/src/nonosctl/omnibridge.rs — Sovereign OmniBridge Relay for NØN-OS
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Securely bridges capsule events, zk identities, and state telemetry across omni mesh nodes

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use sha2::{Digest, Sha256};
use flate2::{Compression, write::GzEncoder};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Keypair, Signer, Signature, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH};

const EVENT_DIR: &str = "/var/nonos/runtime/events";
const TELEMETRY_DIR: &str = "/var/nonos/telemetry";
const RELAY_STATUS: &str = "/var/nonos/bridge/status.json";
const RELAY_QUEUE: &str = "/var/nonos/bridge/queue.json";
const RELAY_KEYS: &str = "/etc/nonos/bridge_key.json";
const MAX_RETRY: usize = 3;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RelayPacket {
    pub id: String,
    pub capsule: String,
    pub kind: String,
    pub timestamp: String,
    pub payload: String,
    pub checksum: String,
    pub signature: String,
    pub attempts: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BridgeStatus {
    pub last_sent: Option<String>,
    pub queue_len: usize,
    pub failures: usize,
    pub relay_peers: Vec<String>,
}

pub fn init_bridge_keypair() {
    if !Path::new(RELAY_KEYS).exists() {
        let mut csprng = rand::rngs::OsRng {};
        let keypair = Keypair::generate(&mut csprng);
        let bytes = keypair.to_bytes();
        fs::write(RELAY_KEYS, general_purpose::STANDARD.encode(&bytes)).expect("[bridge] failed to write bridge key");
        println!("[bridge] keypair generated.");
    }
}

fn load_keypair() -> Option<Keypair> {
    if let Ok(encoded) = fs::read_to_string(RELAY_KEYS) {
        if let Ok(decoded) = general_purpose::STANDARD.decode(encoded.trim()) {
            if decoded.len() == PUBLIC_KEY_LENGTH + SECRET_KEY_LENGTH {
                return Keypair::from_bytes(&decoded).ok();
            }
        }
    }
    None
}

pub fn relay_watcher() {
    fs::create_dir_all("/var/nonos/bridge").ok();
    let bridge_key = load_keypair().expect("[bridge] missing keypair");
    thread::spawn(move || loop {
        process_event_dir(&bridge_key);
        process_telemetry_dir(&bridge_key);
        flush_queue(&bridge_key);
        thread::sleep(Duration::from_secs(10));
    });
}

fn process_event_dir(key: &Keypair) {
    if let Ok(entries) = fs::read_dir(EVENT_DIR) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(json): Result<serde_json::Value, _> = serde_json::from_str(&data) {
                        let capsule = json.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let payload = compress_and_encode(&data);
                        let id = format!("event:{}:{}", capsule, Utc::now().timestamp_nanos());
                        enqueue_packet(capsule, "event", payload, id, key);
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }
}

fn process_telemetry_dir(key: &Keypair) {
    if let Ok(entries) = fs::read_dir(TELEMETRY_DIR) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(data) = fs::read_to_string(&path) {
                    let capsule = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                    let payload = compress_and_encode(&data);
                    let id = format!("telemetry:{}:{}", capsule, Utc::now().timestamp_nanos());
                    enqueue_packet(capsule, "telemetry", payload, id, key);
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

fn enqueue_packet(capsule: &str, kind: &str, payload: String, id: String, key: &Keypair) {
    let checksum = sha256_hash(&payload);
    let msg = format!("{}:{}:{}:{}", capsule, kind, checksum, &payload);
    let sig = key.sign(msg.as_bytes());
    let packet = RelayPacket {
        id,
        capsule: capsule.into(),
        kind: kind.into(),
        timestamp: Utc::now().to_rfc3339(),
        payload,
        checksum,
        signature: hex::encode(sig.to_bytes()),
        attempts: 0,
    };

    let mut queue = read_queue();
    queue.push(packet);
    write_queue(&queue);
}

fn compress_and_encode(data: &str) -> String {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();
    general_purpose::STANDARD.encode(compressed)
}

fn sha256_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn read_queue() -> Vec<RelayPacket> {
    if let Ok(data) = fs::read_to_string(RELAY_QUEUE) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn write_queue(queue: &[RelayPacket]) {
    fs::write(RELAY_QUEUE, serde_json::to_string_pretty(queue).unwrap()).ok();
}

fn flush_queue(key: &Keypair) {
    let mut queue = read_queue();
    let mut sent = 0;
    let mut failed = 0;
    let peers = get_omninet_relays();

    queue.retain(|packet| {
        if let Some(peer) = peers.get(0) { // For now use first peer only
            let result = try_send_to_relay(peer, packet);
            if result {
                sent += 1;
                false
            } else if packet.attempts + 1 >= MAX_RETRY {
                failed += 1;
                false
            } else {
                true
            }
        } else {
            true
        }
    });

    write_queue(&queue);
    write_status(sent, queue.len(), failed, peers);
}

fn try_send_to_relay(_peer: &str, packet: &RelayPacket) -> bool {
    // simulate network send
    println!("[bridge] ⬆️ sending {} to relay...", packet.id);
    true // TODO: implement HTTP / libp2p send
}

fn get_omninet_relays() -> Vec<String> {
    vec!["https://relay.omninet.xyz/api/ingest".into()] // configurable later
}

fn write_status(sent: usize, queued: usize, failed: usize, peers: Vec<String>) {
    let status = BridgeStatus {
        last_sent: Some(Utc::now().to_rfc3339()),
        queue_len: queued,
        failures: failed,
        relay_peers: peers,
    };
    fs::write(RELAY_STATUS, serde_json::to_string_pretty(&status).unwrap()).ok();
}
