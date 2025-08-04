// cli/src/nonosctl/capsule_runtime.rs — NØN Capsule Runtime System
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Handles lifecycle: deploy, run, verify, logs, info, delete

use std::fs;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CAPSULE_DB: &str = "/var/nonos/capsules/index.json";
const CAPSULE_DIR: &str = "/var/nonos/capsules";
const LOG_DIR: &str = "/var/nonos/capsules/logs";
const TELEMETRY_DIR: &str = "/var/nonos/capsules/telemetry";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CapsuleInfo {
    pub api_version: String,
    pub name: String,
    pub path: String,
    pub deployed_at: String,
    pub checksum: String,
    pub mode: String,
    pub permissions: Vec<String>,
}

pub fn deploy_capsule(name: &str, source_path: &str) {
    let target_path = format!("{}/{}", CAPSULE_DIR, name);
    let deployed_at = Utc::now().to_rfc3339();

    if !Path::new(source_path).exists() {
        println!("[capsule] error: source '{}' does not exist.", source_path);
        return;
    }

    fs::create_dir_all(LOG_DIR).ok();
    fs::create_dir_all(TELEMETRY_DIR).ok();
    fs::copy(source_path, &target_path).expect("[capsule] failed to copy binary");

    let checksum = compute_sha256(&target_path).unwrap_or_else(|_| "<error>".into());

    let mut capsule = CapsuleInfo {
        api_version: "v2".into(),
        name: name.into(),
        path: target_path.clone(),
        deployed_at,
        checksum,
        mode: "SAFE".into(),
        permissions: vec!["net".into(), "fs".into()],
    };

    let manifest_path = Path::new(source_path).with_file_name("manifest.toml");
    if manifest_path.exists() {
        if let Ok(contents) = fs::read_to_string(manifest_path) {
            let parsed: toml::Value = toml::from_str(&contents).unwrap_or_default();
            if let Some(mode) = parsed.get("mode").and_then(|v| v.as_str()) {
                capsule.mode = mode.into();
            }
            if let Some(perms) = parsed.get("permissions").and_then(|v| v.as_array()) {
                capsule.permissions = perms.iter().filter_map(|p| p.as_str().map(String::from)).collect();
            }
        }
    }

    let mut index = read_index();
    index.insert(name.into(), capsule.clone());
    fs::write(CAPSULE_DB, serde_json::to_string_pretty(&index).unwrap()).expect("[capsule] failed to write DB");
    println!("[capsule] '{}' deployed successfully.", name);
}

pub fn run_capsule(name: &str) {
    let index = read_index();
    if let Some(info) = index.get(name) {
        println!("[capsule] running '{}'...", name);
        let log_path = format!("{}/{}.log", LOG_DIR, name);
        let telemetry_path = format!("{}/{}.json", TELEMETRY_DIR, name);

        if info.mode == "RAW" && cfg!(feature = "safe") {
            println!("[capsule] execution blocked: '{}' requires RAW mode but system is in SAFE mode.", name);
            return;
        }

        let start_time = Utc::now();
        let result = Command::new(&info.path)
            .env("NONOS_MODE", &info.mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        match result {
            Ok(out) => {
                fs::write(&log_path, &out.stdout).ok();
                fs::write(&telemetry_path, serde_json::json!({
                    "name": name,
                    "exit_code": out.status.code(),
                    "ran_at": start_time.to_rfc3339(),
                    "duration_ms": Utc::now().signed_duration_since(start_time).num_milliseconds()
                }).to_string()).ok();
                rotate_log_if_needed(&log_path);
                println!("[capsule] execution complete.");
            },
            Err(e) => println!("[capsule] error: {}", e),
        }
    } else {
        println!("[capsule] '{}' not found.", name);
    }
}

pub fn verify_capsule(name: &str) {
    let db = read_index();
    if let Some(capsule) = db.get(name) {
        let current = compute_sha256(&capsule.path).unwrap_or_default();
        if current == capsule.checksum {
            println!("[verify] ✅ '{}' passed integrity check.", name);
        } else {
            println!("[verify] ❌ '{}' checksum mismatch.", name);
        }
    } else {
        println!("[verify] capsule '{}' not found.", name);
    }
}

pub fn capsule_info(name: &str, json_out: bool) {
    let index = read_index();
    if let Some(info) = index.get(name) {
        if json_out {
            println!("{}", serde_json::to_string_pretty(info).unwrap());
        } else {
            println!("[capsule] '{}': {:?}", name, info);
        }
    } else {
        println!("[capsule] '{}' not found.", name);
    }
}

pub fn list_capsules(json_out: bool) {
    let index = read_index();
    if json_out {
        println!("{}", serde_json::to_string_pretty(&index).unwrap());
    } else {
        for (name, info) in index.iter() {
            println!("- {} [{}]", name, info.mode);
        }
    }
}

pub fn capsule_logs(name: &str) {
    let log_path = format!("{}/{}.log", LOG_DIR, name);
    if Path::new(&log_path).exists() {
        let content = fs::read_to_string(log_path).unwrap_or_default();
        println!("[logs:{}]\n{}", name, content);
    } else {
        println!("[capsule] no logs found for '{}'.", name);
    }
}

pub fn delete_capsule(name: &str) {
    let mut index = read_index();
    if let Some(info) = index.remove(name) {
        fs::remove_file(&info.path).ok();
        fs::remove_file(format!("{}/{}.log", LOG_DIR, name)).ok();
        fs::remove_file(format!("{}/{}.json", TELEMETRY_DIR, name)).ok();
        let _ = fs::write(CAPSULE_DB, serde_json::to_string_pretty(&index).unwrap());
        println!("[capsule] '{}' deleted.", name);
    } else {
        println!("[capsule] '{}' not found.", name);
    }
}

fn compute_sha256(path: &str) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 4096];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_index() -> HashMap<String, CapsuleInfo> {
    if Path::new(CAPSULE_DB).exists() {
        if let Ok(mut file) = fs::File::open(CAPSULE_DB) {
            let mut data = String::new();
            file.read_to_string(&mut data).ok();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    }
}

fn rotate_log_if_needed(path: &str) {
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() > 1024 * 1024 {
            let _ = fs::rename(path, format!("{}.old", path));
            println!("[log] rotated log for '{}'.", path);
        }
    }
}

