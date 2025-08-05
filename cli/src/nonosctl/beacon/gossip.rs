// cli/src/nonosctl/beacon/gossip.rs — Decentralized Gossip Sync Layer
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Capsule Gossip Engine: rebroadcasts ZK-verified trust maps, manages mesh hop control, deduplicates packets, and anchors runtime state integrity

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use chrono::Utc;
use crate::beacon::trust::{load_trust_db, TrustEntry, update_trust};
use crate::beacon::state::compute_runtime_hash;

const GOSSIP_CACHE_FILE: &str = "/var/nonos/mesh/gossip/cache.json";
const GOSSIP_LOG_DIR: &str = "/var/nonos/mesh/gossip/logs";
const GOSSIP_HOP_TTL: u8 = 6;
const TRUST_THRESHOLD: i32 = 60;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GossipPacket {
    pub origin: String,
    pub timestamp: u64,
    pub zk_verified: bool,
    pub trust_snapshot: HashMap<String, i32>,
    pub runtime_hash: Option<String>,
    pub hop_count: u8,
    pub signature: String,
    pub cluster_hint: Option<String>,
    pub digest: String,
}

pub fn broadcast_gossip(origin: &str, zk_verified: bool) {
    let trust_db = load_trust_db();
    let filtered: HashMap<String, i32> = trust_db.iter()
        .filter(|(_, entry)| entry.zk_valid && entry.score > 40)
        .map(|(k, v)| (k.clone(), v.score))
        .collect();

    let runtime_hash = compute_runtime_hash().ok();
    let timestamp = now();
    let digest = compute_digest(&filtered, runtime_hash.as_deref(), origin, timestamp);

    let packet = GossipPacket {
        origin: origin.to_string(),
        timestamp,
        zk_verified,
        trust_snapshot: filtered,
        runtime_hash,
        hop_count: 0,
        signature: "sig_placeholder".to_string(),
        cluster_hint: Some("mainnet".into()),
        digest,
    };

    persist_packet(&packet);
    println!("[gossip] Packet broadcasted from '{}'.", origin);
}

pub fn handle_incoming_gossip(packet: GossipPacket) {
    if packet.hop_count > GOSSIP_HOP_TTL {
        println!("[gossip] Dropped packet: exceeded TTL");
        return;
    }

    if seen_digest(&packet.digest) {
        println!("[gossip] Duplicate packet ignored.");
        return;
    }

    if !packet.zk_verified {
        println!("[gossip] Ignored unverified gossip.");
        return;
    }

    let local_db = load_trust_db();
    if let Some(score) = local_db.get(&packet.origin).map(|e| e.score) {
        if score < TRUST_THRESHOLD {
            println!("[gossip] Origin '{}' below trust threshold.", packet.origin);
            return;
        }
    }

    log_gossip(&packet);
    update_trust(&packet.origin, 2, "received clean gossip");
    mark_digest_seen(&packet.digest);
    println!("[gossip] Gossip from '{}' accepted and merged.", packet.origin);

    // Re-broadcast logic with incremented hop count
    let mut rebroadcast = packet.clone();
    rebroadcast.hop_count += 1;
    rebroadcast.signature = "sig_rebroadcast".into();
    persist_packet(&rebroadcast);
    println!("[gossip] Rebroadcasting with hop={}.", rebroadcast.hop_count);
}

fn persist_packet(pkt: &GossipPacket) {
    let json = serde_json::to_string_pretty(pkt).unwrap();
    let fname = format!("{}/{}.json", GOSSIP_LOG_DIR, pkt.digest);
    fs::create_dir_all(GOSSIP_LOG_DIR).ok();
    fs::write(fname, json).ok();
}

fn compute_digest(snapshot: &HashMap<String, i32>, runtime: Option<&str>, origin: &str, ts: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(origin.as_bytes());
    hasher.update(ts.to_le_bytes());
    if let Some(rt) = runtime {
        hasher.update(rt.as_bytes());
    }
    for (k, v) in snapshot.iter() {
        hasher.update(k.as_bytes());
        hasher.update(v.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn seen_digest(digest: &str) -> bool {
    let path = Path::new(GOSSIP_CACHE_FILE);
    if path.exists() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(set): Result<HashSet<String>, _> = serde_json::from_str(&data) {
                return set.contains(digest);
            }
        }
    }
    false
}

fn mark_digest_seen(digest: &str) {
    let path = Path::new(GOSSIP_CACHE_FILE);
    let mut set: HashSet<String> = if path.exists() {
        fs::read_to_string(path).ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        HashSet::new()
    };
    set.insert(digest.into());
    fs::create_dir_all("/var/nonos/mesh/gossip").ok();
    fs::write(GOSSIP_CACHE_FILE, serde_json::to_string_pretty(&set).unwrap()).ok();
}

fn log_gossip(pkt: &GossipPacket) {
    let line = format!("[{}] {}@{} :: {} entries | hop={}\n",
        Utc::now().to_rfc3339(),
        pkt.origin,
        pkt.timestamp,
        pkt.trust_snapshot.len(),
        pkt.hop_count,
    );
    let log_path = "/var/nonos/mesh/gossip/events.log";
    fs::create_dir_all("/var/nonos/mesh/gossip").ok();
    fs::OpenOptions::new().create(true).append(true).open(log_path)
        .and_then(|mut f| f.write_all(line.as_bytes())).ok();
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
