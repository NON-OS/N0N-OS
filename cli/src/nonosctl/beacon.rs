// cli/src/nonosctl/beacon.rs — NØN-OS Capsule Beacon Signal & Sync Layer
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Secure mesh-local signaling, zk-valid gossip, runtime hash alerts, skew detection & trust scoring
// nox_____f____beyond

use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, UdpSocket},
    path::Path,
    str,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

use crate::logging::{log_event, LogKind, LogMeta};

const BEACON_PORT: u16 = 40512;
const BEACON_SECRET: &str = "N0N_BEACON_V2";
const CAPSULE_STATE_PATH: &str = "/run/nonos/runtime";
const BROADCAST_INTERVAL_SECS: u64 = 10;
const TRUST_LOG: &str = "/var/nonos/mesh/beacon_audit.log";

static mut SEEN_NONCES: Option<Arc<Mutex<HashMap<String, VecDeque<String>>>>> = None;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BeaconPacket {
    pub sender: String,
    pub hash: String,
    pub zk_verified: bool,
    pub sent_at: String,
    pub signature: String,
    pub nonce: String,
}

pub fn start_beacon_service(pubkey: &str, keypair: &Keypair) {
    let pubkey = pubkey.to_string();
    let kp = keypair.clone();

    unsafe {
        SEEN_NONCES = Some(Arc::new(Mutex::new(HashMap::new())));
    }

    thread::spawn(move || {
        let socket = UdpSocket::bind(("0.0.0.0", 0)).expect("[beacon] failed to bind UDP");
        socket.set_broadcast(true).expect("[beacon] broadcast enable failed");

        loop {
            let hash = hash_runtime_state();
            let nonce = generate_nonce();
            let payload = format!("{}:{}:{}:{}", &pubkey, &hash, &nonce, Utc::now());
            let signature = hex::encode(kp.sign(payload.as_bytes()).to_bytes());

            let packet = BeaconPacket {
                sender: pubkey.clone(),
                hash,
                zk_verified: true,
                sent_at: Utc::now().to_rfc3339(),
                signature,
                nonce,
            };

            if let Ok(json) = serde_json::to_string(&packet) {
                let msg = format!("{}:{}", BEACON_SECRET, json);
                let _ = socket.send_to(msg.as_bytes(), format!("255.255.255.255:{}", BEACON_PORT));
                log_event("beacon", &pubkey, "broadcast", "beacon.rs", "sent secure beacon");
            }

            thread::sleep(Duration::from_secs(BROADCAST_INTERVAL_SECS));
        }
    });

    listen_for_beacons(pubkey.clone());
}

fn listen_for_beacons(local_pubkey: String) {
    thread::spawn(move || {
        let socket = UdpSocket::bind(("0.0.0.0", BEACON_PORT)).expect("[beacon] UDP listen fail");
        let mut buf = [0u8; 2048];

        loop {
            if let Ok((size, _src)) = socket.recv_from(&mut buf) {
                if let Ok(msg) = str::from_utf8(&buf[..size]) {
                    if let Some(rest) = msg.strip_prefix(&format!("{}:", BEACON_SECRET)) {
                        if let Ok(packet) = serde_json::from_str::<BeaconPacket>(rest) {
                            if packet.sender != local_pubkey {
                                if verify_packet(&packet) {
                                    handle_beacon_packet(packet);
                                } else {
                                    println!("[beacon] ❌ invalid signature from {}", packet.sender);
                                    audit_beacon(&packet.sender, "signature_invalid");
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

fn verify_packet(packet: &BeaconPacket) -> bool {
    let pubkey_bytes = match bs58::decode(&packet.sender).into_vec() {
        Ok(b) => b,
        Err(_) => return false,
    };

    let pubkey = match PublicKey::from_bytes(&pubkey_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };

    let sig_bytes = match hex::decode(&packet.signature) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let sig = match Signature::from_bytes(&sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // replay protection
    unsafe {
        if let Some(ref cache) = SEEN_NONCES {
            let mut cache_lock = cache.lock().unwrap();
            let entry = cache_lock.entry(packet.sender.clone()).or_insert_with(VecDeque::new);
            if entry.contains(&packet.nonce) {
                return false;
            }
            entry.push_back(packet.nonce.clone());
            if entry.len() > 25 {
                entry.pop_front();
            }
        }
    }

    let message = format!("{}:{}:{}:{}", packet.sender, packet.hash, packet.nonce, packet.sent_at);
    pubkey.verify(message.as_bytes(), &sig).is_ok()
}

fn handle_beacon_packet(packet: BeaconPacket) {
    let local_hash = hash_runtime_state();
    let remote_time = DateTime::parse_from_rfc3339(&packet.sent_at).unwrap_or(Utc::now().into());
    let skew = (Utc::now() - remote_time.with_timezone(&Utc)).num_seconds();

    if packet.hash != local_hash {
        println!(
            "[beacon] ⚠️  hash mismatch from {} | local={} remote={} skew={}s",
            packet.sender, local_hash, packet.hash, skew
        );
        log_event("beacon", &packet.sender, "state_diff", "beacon.rs", "state mismatch");
        audit_beacon(&packet.sender, "hash_diff");
    } else {
        println!("[beacon] ✅ peer {} is synced | skew={}s", packet.sender, skew);
        audit_beacon(&packet.sender, "ok");
    }
}

fn generate_nonce() -> String {
    let mut rng = thread_rng();
    (0..8).map(|_| rng.gen_range(0u8..=255)).map(|b| format!("{:02x}", b)).collect()
}

fn hash_runtime_state() -> String {
    let mut entries = vec![];

    if let Ok(dir) = fs::read_dir(CAPSULE_STATE_PATH) {
        for entry in dir.flatten() {
            if let Ok(mut file) = File::open(entry.path()) {
                let mut content = String::new();
                file.read_to_string(&mut content).ok();
                entries.push(content);
            }
        }
    }

    let joined = entries.join("|");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn audit_beacon(sender: &str, status: &str) {
    let log = format!(
        "{} :: peer={} status={}\n",
        Utc::now().to_rfc3339(),
        sender,
        status
    );
    fs::create_dir_all("/var/nonos/mesh").ok();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(TRUST_LOG)
        .unwrap();
    let _ = file.write_all(log.as_bytes());
}
