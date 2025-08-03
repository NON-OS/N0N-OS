// cli/src/nonosctl/capsules.rs — NØN-OS Capsule Operations (Advanced Execution + Telemetry)

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use sha2::{Sha256, Digest};
use chrono::Utc;

const CAPSULE_DIR: &str = "/var/nonos/capsules/";
const CAPSULE_INDEX: &str = "/var/nonos/runtime/capsule_index.json";
const CAPSULE_LOG_DIR: &str = "/var/nonos/logs/";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapsuleMeta {
    pub name: String,
    pub version: String,
    pub hash: String,
    pub path: String,
    pub deployed: bool,
    pub last_updated: String,
    pub tags: Option<HashMap<String, String>>,
}

pub fn deploy_capsule(name: &str, path: &str) {
    let dest_path = format!("{}{}", CAPSULE_DIR, name);
    match fs::copy(path, &dest_path) {
        Ok(_) => {
            let hash = calculate_hash(&dest_path);
            let meta = CapsuleMeta {
                name: name.into(),
                version: "1.0.0".into(),
                hash,
                path: dest_path.clone(),
                deployed: true,
                last_updated: Utc::now().to_rfc3339(),
                tags: Some(HashMap::new()),
            };
            store_capsule_meta(name, &meta);
            println!("[capsule] '{}' deployed successfully.", name);
        }
        Err(e) => println!("[capsule] deploy failed: {}", e),
    }
}

pub fn run_capsule(name: &str, args: &[&str]) {
    if let Some(meta) = load_capsule_meta(name) {
        if Path::new(&meta.path).exists() {
            match Command::new(&meta.path)
                .args(args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn() {
                Ok(mut child) => {
                    let _ = child.wait();
                }
                Err(e) => println!("[capsule] failed to execute '{}': {}", name, e),
            }
        } else {
            println!("[capsule] binary '{}' missing at path: {}", name, meta.path);
        }
    } else {
        println!("[capsule] '{}' not found.", name);
    }
}

pub fn capsule_logs(name: &str) {
    let log_path = format!("{}{}.log", CAPSULE_LOG_DIR, name);
    match fs::read_to_string(&log_path) {
        Ok(content) => println!("{}", content),
        Err(_) => println!("[capsule] no logs found for '{}'.", name),
    }
}

pub fn stream_capsule(name: &str) {
    let log_path = format!("{}{}.log", CAPSULE_LOG_DIR, name);
    if !Path::new(&log_path).exists() {
        println!("[capsule] log file not found for '{}'.", name);
        return;
    }
    let _ = Command::new("tail")
        .arg("-f")
        .arg(log_path)
        .status();
}

pub fn search_capsules(keyword: &str) {
    let index = load_capsule_index();
    let results: Vec<_> = index.iter()
        .filter(|(_, meta)|
            meta.name.contains(keyword)
            || meta.hash.contains(keyword)
            || meta.version.contains(keyword))
        .collect();

    if results.is_empty() {
        println!("[capsule] no matches found for '{}'.", keyword);
    } else {
        for (k, v) in results {
            println!("[capsule] match '{}': {} v{}", k, v.hash, v.version);
        }
    }
}

pub fn tag_capsule(name: &str, key: &str, value: &str) {
    let mut index = load_capsule_index();
    if let Some(meta) = index.get_mut(name) {
        if let Some(tags) = &mut meta.tags {
            tags.insert(key.to_string(), value.to_string());
            save_capsule_index(&index);
            println!("[capsule] tag added: {}={}", key, value);
        }
    } else {
        println!("[capsule] '{}' not found for tagging.", name);
    }
}

pub fn verify_capsule(name: &str) {
    if let Some(meta) = load_capsule_meta(name) {
        let actual_hash = calculate_hash(&meta.path);
        if actual_hash == meta.hash {
            println!("[capsule] '{}' verified OK.", name);
        } else {
            println!("[capsule] '{}' integrity FAILED.", name);
        }
    }
}

pub fn inspect_capsule(name: &str) {
    let meta = load_capsule_meta(name);
    match meta {
        Some(m) => {
            println!("[capsule] '{}':", name);
            println!(" - version: {}", m.version);
            println!(" - hash: {}", m.hash);
            println!(" - path: {}", m.path);
            println!(" - deployed: {}", m.deployed);
            println!(" - last updated: {}", m.last_updated);
            if let Some(tags) = m.tags {
                for (k, v) in tags.iter() {
                    println!("   [tag] {} = {}", k, v);
                }
            }
        }
        None => println!("[capsule] '{}' not found in index.", name),
    }
}

pub fn remove_capsule(name: &str) {
    if let Some(meta) = load_capsule_meta(name) {
        if fs::remove_file(&meta.path).is_ok() {
            remove_capsule_meta(name);
            println!("[capsule] '{}' removed from system.", name);
        } else {
            println!("[capsule] failed to remove '{}'.", name);
        }
    }
}

fn calculate_hash(path: &str) -> String {
    let mut file = File::open(path).expect("Unable to open file for hash");
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 4096];
    loop {
        let bytes_read = file.read(&mut buffer).unwrap_or(0);
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    format!("{:x}", hasher.finalize())
}

fn store_capsule_meta(name: &str, meta: &CapsuleMeta) {
    let mut index = load_capsule_index();
    index.insert(name.to_string(), meta.clone());
    save_capsule_index(&index);
}

fn load_capsule_meta(name: &str) -> Option<CapsuleMeta> {
    load_capsule_index().get(name).cloned()
}

fn remove_capsule_meta(name: &str) {
    let mut index = load_capsule_index();
    index.remove(name);
    save_capsule_index(&index);
}

fn load_capsule_index() -> HashMap<String, CapsuleMeta> {
    if let Ok(json) = fs::read_to_string(CAPSULE_INDEX) {
        serde_json::from_str(&json).unwrap_or_default()
    } else {
        HashMap::new()
    }
}

fn save_capsule_index(index: &HashMap<String, CapsuleMeta>) {
    if let Ok(json) = serde_json::to_string_pretty(index) {
        let _ = fs::write(CAPSULE_INDEX, json);
    }
}

