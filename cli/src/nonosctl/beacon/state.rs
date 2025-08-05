// cli/src/nonosctl/beacon/state.rs — Runtime State Snapshot + Merkle Summary + Change Tracking
// Ⓝ NØN-OS Sovereign Capsule OS | Maintained by ek@nonos-tech.xyz
// Built with zero tolerance for surveillance, full commitment to decentralization.
// "Privacy is a non-negotiable human right — this system is yours, not theirs."
//
// © 2025 NØN Technologies. All rights reserved.

use std::fs::{self, File, Metadata};
use std::io::{Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use base58::ToBase58;

const CAPSULE_STATE_PATH: &str = "/run/nonos/runtime";
const SNAPSHOT_DIR: &str = "/var/nonos/snapshots";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StateEntry {
    pub file: String,
    pub mtime: u64,
    pub size: u64,
    pub hash: String,
    pub ftype: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub hash: String,
    pub entries: Vec<StateEntry>,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StateDiff {
    pub added: Vec<StateEntry>,
    pub removed: Vec<String>,
    pub changed: Vec<(String, String)>, // filename -> new hash
}

/// Computes Merkle-style hash of sorted state + returns detailed entries
pub fn hash_runtime_state_detailed() -> StateSnapshot {
    let mut entries = vec![];

    if let Ok(dir) = fs::read_dir(CAPSULE_STATE_PATH) {
        for entry in dir.flatten() {
            let path = entry.path();
            if let Ok(mut file) = File::open(&path) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    let meta = fs::metadata(&path).unwrap_or_else(|_| fake_metadata());
                    let hash = {
                        let mut h = Sha256::new();
                        h.update(content.as_bytes());
                        format!("{:x}", h.finalize())
                    };

                    entries.push(StateEntry {
                        file: path.file_name().unwrap().to_string_lossy().to_string(),
                        mtime: meta.modified().ok().and_then(to_epoch).unwrap_or(0),
                        size: meta.len(),
                        hash,
                        ftype: detect_file_type(&path),
                    });
                }
            }
        }
    }

    entries.sort_by_key(|e| e.file.clone());
    let mut full_hasher = Sha256::new();
    for entry in &entries {
        full_hasher.update(format!("{}:{}:{}:{}:{}|", entry.file, entry.size, entry.mtime, entry.ftype, entry.hash));
    }

    let root_hash = format!("{:x}", full_hasher.finalize());

    StateSnapshot {
        hash: root_hash,
        entries,
        timestamp: now_epoch(),
    }
}

/// Save the current state snapshot to disk for auditing
pub fn export_state_snapshot(snapshot: &StateSnapshot) {
    fs::create_dir_all(SNAPSHOT_DIR).ok();
    let fname = format!("{}/{}.json", SNAPSHOT_DIR, snapshot.hash);
    if let Ok(json) = serde_json::to_string_pretty(snapshot) {
        fs::write(fname, json).ok();
    }
}

/// Compute diff between two runtime state snapshots
pub fn diff_snapshots(old: &StateSnapshot, new: &StateSnapshot) -> StateDiff {
    let mut old_map: HashMap<String, &StateEntry> = old.entries.iter().map(|e| (e.file.clone(), e)).collect();
    let mut new_map: HashMap<String, &StateEntry> = new.entries.iter().map(|e| (e.file.clone(), e)).collect();

    let mut added = vec![];
    let mut removed = vec![];
    let mut changed = vec![];

    for (k, v) in new_map.iter() {
        if !old_map.contains_key(k) {
            added.push((*v).clone());
        } else if old_map[k].hash != v.hash {
            changed.push((k.clone(), v.hash.clone()));
        }
    }

    for k in old_map.keys() {
        if !new_map.contains_key(k) {
            removed.push(k.clone());
        }
    }

    StateDiff { added, removed, changed }
}

fn detect_file_type(path: &Path) -> String {
    if let Some(ext) = path.extension() {
        ext.to_string_lossy().to_string()
    } else {
        "unknown".into()
    }
}

fn fake_metadata() -> Metadata {
    fs::metadata("/dev/null").unwrap_or_else(|_| panic!("no metadata"))
}

fn to_epoch(st: SystemTime) -> Option<u64> {
    st.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

fn now_epoch() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Utility for CID-style hash representation
pub fn hash_base58(hash: &str) -> String {
    hash.as_bytes().to_base58()
}
