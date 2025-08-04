// cli/src/nonosctl/kernel.rs — Core Kernel Interface for NØN-OS Runtime
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Kernel insights, config verification, patch view, and snapshot export

use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process::Command;
use serde::{Serialize, Deserialize};
use chrono::{Utc, DateTime};

const KERNEL_CONFIG: &str = "/etc/nonos/config.toml";
const PATCH_FILE: &str = "/var/nonos/kernel/patches.json";
const MODULES_DIR: &str = "/usr/lib/nonos/modules";
const SNAPSHOT_PATH: &str = "/var/nonos/reports/kernel_snapshot.json";
const KERNEL_VERSION: &str = "0.8.12-nonos";

#[derive(Debug, Serialize, Deserialize)]
pub struct KernelInfo {
    pub version: String,
    pub build_date: String,
    pub host: String,
    pub platform: String,
    pub uptime: String,
    pub memory: HashMap<String, String>,
    pub config: Option<HashMap<String, String>>,
    pub patches: Option<Vec<String>>,
    pub modules: Option<Vec<String>>,
    pub lock_level: Option<String>,
    pub diagnostics: Option<HashMap<String, String>>,
}

pub fn print_kernel_info(json: bool) {
    let info = collect_kernel_info();
    if json {
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
    } else {
        println!("[nonos-kernel] {} (build {})", info.version, info.build_date);
        println!("host: {} | platform: {}", info.host, info.platform);
        println!("uptime: {}", info.uptime);
        println!("memory: {:?}", info.memory);
        if let Some(cfg) = info.config.clone() {
            println!("config: {:?}", cfg);
        }
        if let Some(p) = &info.patches {
            println!("patches: {:?}", p);
        }
        if let Some(m) = &info.modules {
            println!("modules: {:?}", m);
        }
        if let Some(lock) = &info.lock_level {
            println!("lock level: {}", lock);
        }
    }
}

pub fn export_snapshot() {
    let info = collect_kernel_info();
    if let Some(parent) = Path::new(SNAPSHOT_PATH).parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(&info) {
        if fs::write(SNAPSHOT_PATH, json).is_ok() {
            println!("[kernel] snapshot exported to '{}'.", SNAPSHOT_PATH);
        } else {
            println!("[kernel] failed to write snapshot file.");
        }
    }
}

fn collect_kernel_info() -> KernelInfo {
    let config = read_config_map();
    KernelInfo {
        version: KERNEL_VERSION.into(),
        build_date: Utc::now().format("%Y-%m-%d").to_string(),
        host: hostname::get().unwrap_or_default().to_string_lossy().into(),
        platform: whoami::platform().to_string(),
        uptime: get_uptime_string(),
        memory: parse_meminfo(),
        config: config.clone(),
        patches: read_patch_list(),
        modules: scan_modules(),
        lock_level: config.as_ref().and_then(|c| c.get("lock_level").cloned()),
        diagnostics: kernel_diagnostics(),
    }
}

fn get_uptime_string() -> String {
    match fs::read_to_string("/proc/uptime") {
        Ok(data) => {
            let seconds = data.split_whitespace().next().unwrap_or("0").parse::<u64>().unwrap_or(0);
            let hours = seconds / 3600;
            let mins = (seconds % 3600) / 60;
            format!("{}h {}m", hours, mins)
        },
        Err(_) => "n/a".into(),
    }
}

fn parse_meminfo() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once(":") {
                map.insert(k.trim().into(), v.trim().into());
            }
        }
    }
    map
}

fn read_config_map() -> Option<HashMap<String, String>> {
    if !Path::new(KERNEL_CONFIG).exists() {
        return None;
    }
    let content = fs::read_to_string(KERNEL_CONFIG).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;
    let mut flat = HashMap::new();
    if let Some(tbl) = parsed.as_table() {
        for (k, v) in tbl {
            flat.insert(k.clone(), v.to_string());
        }
    }
    Some(flat)
}

fn read_patch_list() -> Option<Vec<String>> {
    if Path::new(PATCH_FILE).exists() {
        let json = fs::read_to_string(PATCH_FILE).ok()?;
        let parsed: Vec<String> = serde_json::from_str(&json).ok()?;
        Some(parsed)
    } else {
        None
    }
}

fn scan_modules() -> Option<Vec<String>> {
    if !Path::new(MODULES_DIR).exists() {
        return None;
    }
    let mut modules = vec![];
    if let Ok(entries) = fs::read_dir(MODULES_DIR) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                modules.push(name.to_string());
            }
        }
    }
    Some(modules)
}

fn kernel_diagnostics() -> Option<HashMap<String, String>> {
    let mut diag = HashMap::new();
    let sys = Path::new("/sys/kernel");
    if sys.exists() {
        for sub in ["hostname", "random", "mm"].iter() {
            let p = sys.join(sub);
            if p.exists() {
                diag.insert((*sub).into(), "ok".into());
            } else {
                diag.insert((*sub).into(), "missing".into());
            }
        }
        Some(diag)
    } else {
        None
    }
}

