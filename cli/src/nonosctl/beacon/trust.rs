// cli/src/nonosctl/beacon/trust.rs — Decentralized Capsule Trust Engine
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Proof of Decentralization Layer: trust scoring, zk verification cache, reputation roles, mesh exchange, anomaly detection

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use chrono::Utc;

const TRUST_DB: &str = "/var/nonos/mesh/trust/scores.json";
const TRUST_EVENTS: &str = "/var/nonos/mesh/trust/events";
const MAX_EVENTS: usize = 100;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PeerStatus {
    Trusted,
    Unknown,
    Flagged,
    Blacklisted,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrustEntry {
    pub pubkey: String,
    pub score: i32,
    pub federated_score: Option<i32>,
    pub status: PeerStatus,
    pub last_seen: u64,
    pub last_latency_ms: Option<u32>,
    pub zk_valid: bool,
    pub zk_verified_at: Option<u64>,
    pub zk_proof_id: Option<String>,
    pub manual_override: bool,
    pub role: Option<String>,
    pub tags: Vec<String>,
    pub successful_sessions: u32,
    pub failures: u32,
    pub history: VecDeque<TrustEvent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrustEvent {
    pub time: u64,
    pub action: String,
    pub reason: String,
    pub delta: i32,
    pub resulting_score: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TrustPolicy {
    Strict,
    Open,
    Adaptive,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MeshTrustExchange {
    pub origin: String,
    pub timestamp: u64,
    pub trust_map: HashMap<String, i32>,
    pub zk_summary: Option<String>,
}

pub fn load_trust_db() -> HashMap<String, TrustEntry> {
    if Path::new(TRUST_DB).exists() {
        if let Ok(json) = fs::read_to_string(TRUST_DB) {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    }
}

pub fn save_trust_db(map: &HashMap<String, TrustEntry>) {
    fs::create_dir_all("/var/nonos/mesh/trust").ok();
    if let Ok(json) = serde_json::to_string_pretty(map) {
        fs::write(TRUST_DB, json).ok();
    }
}

pub fn update_trust(pubkey: &str, delta: i32, reason: &str) {
    let mut db = load_trust_db();
    let now = now_epoch();

    let entry = db.entry(pubkey.into()).or_insert_with(|| TrustEntry {
        pubkey: pubkey.into(),
        score: 50,
        federated_score: None,
        status: PeerStatus::Unknown,
        last_seen: now,
        last_latency_ms: None,
        zk_valid: false,
        zk_verified_at: None,
        zk_proof_id: None,
        manual_override: false,
        role: None,
        tags: vec![],
        successful_sessions: 0,
        failures: 0,
        history: VecDeque::new(),
    });

    entry.last_seen = now;
    entry.score += delta;
    if entry.score > 100 { entry.score = 100; }
    if entry.score < -50 { entry.status = PeerStatus::Blacklisted; }

    let ev = TrustEvent {
        time: now,
        action: "adjust_score".into(),
        reason: reason.into(),
        delta,
        resulting_score: entry.score,
    };
    entry.history.push_back(ev.clone());
    if entry.history.len() > MAX_EVENTS {
        entry.history.pop_front();
    }
    log_trust_event(pubkey, &ev);
    save_trust_db(&db);
}

pub fn tag_peer(pubkey: &str, tag: &str) {
    let mut db = load_trust_db();
    if let Some(entry) = db.get_mut(pubkey) {
        if !entry.tags.contains(&tag.to_string()) {
            entry.tags.push(tag.to_string());
        }
    }
    save_trust_db(&db);
}

pub fn apply_trust_policy(policy: TrustPolicy, peer: &TrustEntry) -> bool {
    match policy {
        TrustPolicy::Strict => peer.score >= 75 && peer.zk_valid,
        TrustPolicy::Open => peer.status != PeerStatus::Blacklisted,
        TrustPolicy::Adaptive => peer.score > 40 || peer.zk_valid,
    }
}

pub fn merge_trust_snapshot(from_peer: &str, data: HashMap<String, TrustEntry>) {
    let mut local = load_trust_db();
    for (k, remote) in data.iter() {
        let e = local.entry(k.clone()).or_insert(remote.clone());
        if remote.score > e.score {
            e.score = remote.score;
            e.last_seen = remote.last_seen;
            e.zk_verified_at = remote.zk_verified_at;
        }
    }
    update_trust(from_peer, 2, "merged trust snapshot");
    save_trust_db(&local);
}

pub fn decay_trust_over_time() {
    let mut db = load_trust_db();
    let now = now_epoch();
    for (_, entry) in db.iter_mut() {
        let since = now - entry.last_seen;
        if since > 3600 && entry.score > 10 {
            entry.score -= 1;
        }
    }
    save_trust_db(&db);
}

pub fn detect_anomalies() {
    let db = load_trust_db();
    for (key, entry) in db.iter() {
        if entry.history.len() >= 2 {
            let recent = &entry.history.back().unwrap();
            let prev = &entry.history.iter().rev().nth(1).unwrap();
            if (recent.resulting_score - prev.resulting_score).abs() > 25 {
                log_trust_event(key, &TrustEvent {
                    time: now_epoch(),
                    action: "anomaly_detected".into(),
                    reason: "sudden trust shift".into(),
                    delta: 0,
                    resulting_score: recent.resulting_score,
                });
            }
        }
    }
}

fn log_trust_event(pubkey: &str, ev: &TrustEvent) {
    let log_path = format!("{}/{}.log", TRUST_EVENTS, pubkey);
    fs::create_dir_all(TRUST_EVENTS).ok();
    let line = format!("[{}] {} (+{}) => {} :: {}\n", ev.time, ev.action, ev.delta, ev.resulting_score, ev.reason);
    fs::OpenOptions::new().create(true).append(true).open(log_path)
        .and_then(|mut f| f.write_all(line.as_bytes())).ok();
}

fn now_epoch() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
