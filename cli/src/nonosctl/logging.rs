// cli/src/nonosctl/logging.rs — NØN-OS Sovereign Audit Engine w/ Hash Chain

use chrono::{Utc};
use serde::{Serialize, Deserialize};
use std::fs::{self, OpenOptions};
use std::io::{BufReader, BufRead, Write};
use std::collections::HashMap;
use sha2::{Sha256, Digest};

const AUDIT_LOG: &str = "/var/nonos/logs/audit.log";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: String,
    pub scope: String,
    pub actor: String,
    pub action: String,
    pub source: String,
    pub detail: String,
    pub hash: String,
    pub prev_hash: String,
}

pub fn log_event(scope: &str, actor: &str, action: &str, source: &str, detail: &str) {
    let prev_hash = get_last_hash();
    let raw_data = format!("{}:{}:{}:{}:{}:{}:{}", Utc::now(), scope, actor, action, source, detail, prev_hash);
    let hash = format!("{:x}", Sha256::digest(raw_data.as_bytes()));

    let event = Event {
        timestamp: Utc::now().to_rfc3339(),
        scope: scope.to_string(),
        actor: actor.to_string(),
        action: action.to_string(),
        source: source.to_string(),
        detail: detail.to_string(),
        hash,
        prev_hash,
    };

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(AUDIT_LOG) {
        if let Ok(json) = serde_json::to_string(&event) {
            let _ = writeln!(file, "{}", json);
        }
    }
}

fn get_last_hash() -> String {
    if let Ok(file) = fs::File::open(AUDIT_LOG) {
        let reader = BufReader::new(file);
        if let Some(last_line) = reader.lines().flatten().last() {
            if let Ok(evt) = serde_json::from_str::<Event>(&last_line) {
                return evt.hash;
            }
        }
    }
    "GENESIS".to_string()
}

pub fn view_audit_log(limit: usize) {
    if let Ok(file) = fs::File::open(AUDIT_LOG) {
        let reader = BufReader::new(file);
        let lines: Vec<_> = reader.lines().filter_map(Result::ok).collect();

        for line in lines.iter().rev().take(limit).rev() {
            if let Ok(evt) = serde_json::from_str::<Event>(line) {
                println!("[{}][{}][{}][{}] {} => {} | #{} <- {}", evt.timestamp, evt.scope, evt.source, evt.actor, evt.action, evt.detail, &evt.hash[..8], &evt.prev_hash[..8]);
            }
        }
    } else {
        println!("[log] audit log not found.");
    }
}

pub fn flush_audit_log() {
    if fs::write(AUDIT_LOG, b"").is_ok() {
        println!("[log] audit log flushed.");
    } else {
        println!("[log] failed to flush audit log.");
    }
}

pub fn filter_by_scope(scope: &str) {
    if let Ok(file) = fs::File::open(AUDIT_LOG) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Ok(evt) = serde_json::from_str::<Event>(&line) {
                if evt.scope == scope {
                    println!("[{}][{}][{}][{}] {} => {}", evt.timestamp, evt.scope, evt.source, evt.actor, evt.action, evt.detail);
                }
            }
        }
    }
}

pub fn filter_by_actor(actor: &str) {
    if let Ok(file) = fs::File::open(AUDIT_LOG) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Ok(evt) = serde_json::from_str::<Event>(&line) {
                if evt.actor == actor {
                    println!("[{}][{}][{}][{}] {} => {}", evt.timestamp, evt.scope, evt.source, evt.actor, evt.action, evt.detail);
                }
            }
        }
    }
}

pub fn audit_stats() {
    let mut totals: HashMap<String, usize> = HashMap::new();
    let mut failures = 0;

    if let Ok(file) = fs::File::open(AUDIT_LOG) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Ok(evt) = serde_json::from_str::<Event>(&line) {
                *totals.entry(evt.scope.clone()).or_insert(0) += 1;
                if evt.action.contains("fail") || evt.detail.contains("denied") {
                    failures += 1;
                }
            }
        }
    }

    println!("[stats] total scopes:");
    for (scope, count) in totals {
        println!("  {}: {} events", scope, count);
    }
    println!("[stats] failure-related events: {}", failures);
}

pub fn export_audit_log(output_path: &str) {
    if let Ok(content) = fs::read_to_string(AUDIT_LOG) {
        if fs::write(output_path, content).is_ok() {
            println!("[log] exported audit log to '{}'.", output_path);
        } else {
            println!("[log] failed to write export.");
        }
    } else {
        println!("[log] audit log missing, cannot export.");
    }
}

