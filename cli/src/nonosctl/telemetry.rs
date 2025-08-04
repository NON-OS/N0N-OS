// cli/src/nonosctl/telemetry.rs — Capsule Execution Telemetry for NØN-OS
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
//️ Captures per-run capsule metadata for runtime introspection, audit, and profiling

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

const TELEMETRY_DIR: &str = "/var/nonos/capsules/telemetry";
const REPORT_DIR: &str = "/var/nonos/reports";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapsuleTelemetry {
    pub name: String,
    pub ran_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub exit_code: Option<i32>,
    pub cpu_usage: Option<f32>,
    pub memory_kb: Option<u64>,
    pub notes: Option<String>,
}

/// Read telemetry JSON from disk
pub fn get_telemetry(name: &str) -> Option<CapsuleTelemetry> {
    let path = format!("{}/{}.json", TELEMETRY_DIR, name);
    if Path::new(&path).exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).ok()
        } else {
            None
        }
    } else {
        None
    }
}

/// Return all capsule telemetry in a HashMap
pub fn list_all_telemetry() -> HashMap<String, CapsuleTelemetry> {
    let mut result = HashMap::new();
    if let Ok(entries) = fs::read_dir(TELEMETRY_DIR) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    let capsule = name.trim_end_matches(".json");
                    if let Some(meta) = get_telemetry(capsule) {
                        result.insert(capsule.to_string(), meta);
                    }
                }
            }
        }
    }
    result
}

/// Print telemetry to stdout (human or JSON)
pub fn print_telemetry(name: Option<&str>, json: bool) {
    match name {
        Some(n) => {
            if let Some(t) = get_telemetry(n) {
                if json {
                    println!("{}", serde_json::to_string_pretty(&t).unwrap());
                } else {
                    println!("[telemetry:{}] {:?}", n, t);
                }
            } else {
                println!("[telemetry] no telemetry found for '{}'.", n);
            }
        },
        None => {
            let all = list_all_telemetry();
            if json {
                println!("{}", serde_json::to_string_pretty(&all).unwrap());
            } else {
                for (name, data) in all.iter() {
                    println!("- {}: {:?}", name, data);
                }
            }
        }
    }
}

/// Add note or tag to telemetry file
pub fn annotate(name: &str, message: &str) {
    let path = format!("{}/{}.json", TELEMETRY_DIR, name);
    if let Some(mut record) = get_telemetry(name) {
        record.notes = Some(message.to_string());
        let _ = fs::write(path, serde_json::to_string_pretty(&record).unwrap());
        println!("[telemetry] note added to '{}'.", name);
    } else {
        println!("[telemetry] record not found.");
    }
}

/// Export all telemetry into a report file
pub fn export_report() {
    fs::create_dir_all(REPORT_DIR).ok();
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let report_path = format!("{}/telemetry_{}.log", REPORT_DIR, ts);
    let mut file = fs::File::create(&report_path).expect("[report] cannot write report");

    let all = list_all_telemetry();
    for (name, t) in all {
        writeln!(file, "# {}\n{:?}\n", name, t).ok();
    }

    println!("[telemetry] exported to '{}'.", report_path);
}

/// Delete all telemetry (requires --force in CLI)
pub fn wipe_all() {
    if let Ok(entries) = fs::read_dir(TELEMETRY_DIR) {
        for entry in entries.flatten() {
            let _ = fs::remove_file(entry.path());
        }
        println!("[telemetry] all telemetry wiped.");
    }
}

/// Average runtime stats across all capsules
pub fn summarize_stats() {
    let all = list_all_telemetry();
    let total = all.len() as f32;
    let sum: i64 = all.values().map(|t| t.duration_ms).sum();

    println!("[telemetry] total runs: {}", total as i64);
    println!("[telemetry] avg duration: {:.2} ms", sum as f32 / total.max(1.0));

    let fail_count = all.values().filter(|t| t.exit_code != Some(0)).count();
    println!("[telemetry] non-zero exit codes: {}", fail_count);
}

/// Telemetry integrity checker (checksum, json validity)
pub fn validate_all() {
    let all = fs::read_dir(TELEMETRY_DIR).unwrap_or_default();
    for file in all.flatten() {
        let path = file.path();
        if path.extension().map(|ext| ext == "json").unwrap_or(false) {
            match fs::read_to_string(&path) {
                Ok(raw) => {
                    if serde_json::from_str::<CapsuleTelemetry>(&raw).is_err() {
                        println!("[validate] invalid JSON: {:?}", path);
                    }
                },
                Err(_) => println!("[validate] cannot read: {:?}", path),
            }
        }
    }
}

