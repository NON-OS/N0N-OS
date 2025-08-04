// cli/src/nonosctl/daemon.rs — Runtime Daemon Engine for NØN-OS
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Nonos, System supervisor with health sync, capsule restart, telemetry, and alerts

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use std::io::Write;
use std::collections::HashMap;
use chrono::Utc;
use serde::{Serialize, Deserialize};
use std::process::{Command, Stdio};

const DAEMON_LOG: &str = "/var/log/nonosd.log";
const WATCH_INTERVAL: u64 = 10; // seconds
const CAPSULE_DB: &str = "/var/nonos/capsules/index.json";
const HEARTBEAT_PATH: &str = "/var/nonos/daemon/heartbeat.json";
const ALERT_DIR: &str = "/var/nonos/alerts";
const CONFIG_PATH: &str = "/etc/nonos/nonosd.toml";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapsuleEntry {
    pub name: String,
    pub path: String,
    pub mode: String,
    pub permissions: Vec<String>,
    pub auto_restart: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct DaemonHeartbeat {
    timestamp: String,
    watched: usize,
    restarts: usize,
    daemon_pid: u32,
}

pub fn start_daemon(verbose: bool) {
    log("nonosd daemon started.");
    let mut restart_count = 0;

    loop {
        let now = Utc::now().to_rfc3339();
        let capsules = load_capsules();
        let mut restarts = 0;

        for c in &capsules {
            let ok = check_capsule_health(&c);
            if verbose {
                println!("[{}] [{}] capsule '{}' health = {}", now, c.mode, c.name, ok);
            }
            if !ok && c.auto_restart {
                if restart_capsule(c) {
                    restarts += 1;
                    log(&format!("capsule '{}' auto-restarted.", c.name));
                } else {
                    write_alert(&c.name, "restart_failed");
                }
            }
        }

        write_heartbeat(capsules.len(), restart_count + restarts);
        restart_count += restarts;

        if let Some(extra) = check_config_flag("log_metrics") {
            if extra == "true" {
                log_metrics();
            }
        }

        thread::sleep(Duration::from_secs(WATCH_INTERVAL));
    }
}

fn load_capsules() -> Vec<CapsuleEntry> {
    if let Ok(data) = fs::read_to_string(CAPSULE_DB) {
        let parsed: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
        parsed.as_object().map(|map| {
            map.iter().filter_map(|(name, v)| {
                let path = v.get("path")?.as_str()?.to_string();
                let mode = v.get("mode").map(|m| m.as_str().unwrap_or("SAFE")).unwrap().to_string();
                let permissions = v.get("permissions").and_then(|p| p.as_array()).map(|arr| {
                    arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()
                }).unwrap_or_default();
                let auto_restart = v.get("auto_restart").and_then(|b| b.as_bool()).unwrap_or(false);
                Some(CapsuleEntry { name: name.clone(), path, mode, permissions, auto_restart })
            }).collect()
        }).unwrap_or_default()
    } else {
        vec![]
    }
}

fn check_capsule_health(capsule: &CapsuleEntry) -> bool {
    Path::new(&capsule.path).exists()
}

fn restart_capsule(capsule: &CapsuleEntry) -> bool {
    Command::new(&capsule.path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

fn write_heartbeat(watched: usize, restarts: usize) {
    let data = DaemonHeartbeat {
        timestamp: Utc::now().to_rfc3339(),
        watched,
        restarts,
        daemon_pid: std::process::id(),
    };
    if let Some(p) = Path::new(HEARTBEAT_PATH).parent() {
        fs::create_dir_all(p).ok();
    }
    let _ = fs::write(HEARTBEAT_PATH, serde_json::to_string_pretty(&data).unwrap());
}

fn write_alert(name: &str, reason: &str) {
    fs::create_dir_all(ALERT_DIR).ok();
    let file = format!("{}/{}_{}.json", ALERT_DIR, name, Utc::now().timestamp());
    let content = serde_json::json!({
        "capsule": name,
        "reason": reason,
        "at": Utc::now().to_rfc3339(),
    });
    let _ = fs::write(file, serde_json::to_string_pretty(&content).unwrap());
}

fn check_config_flag(key: &str) -> Option<String> {
    if Path::new(CONFIG_PATH).exists() {
        let contents = fs::read_to_string(CONFIG_PATH).ok()?;
        let parsed: toml::Value = toml::from_str(&contents).ok()?;
        parsed.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    } else {
        None
    }
}

fn log_metrics() {
    let uptime = fs::read_to_string("/proc/uptime").unwrap_or("0".into());
    let mem = fs::read_to_string("/proc/meminfo").unwrap_or("no meminfo".into());
    log(&format!("[metrics] uptime={} | mem={}", uptime.split_whitespace().next().unwrap_or("0"), mem.lines().next().unwrap_or("none")));
}

fn log(msg: &str) {
    let line = format!("[{}] {}\n", Utc::now().to_rfc3339(), msg);
    let _ = fs::OpenOptions::new().create(true).append(true).open(DAEMON_LOG)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

pub fn status_report() {
    if let Ok(data) = fs::read_to_string(DAEMON_LOG) {
        println!("[nonosd] log:\n{}", data);
    } else {
        println!("[nonosd] no recent daemon log.");
    }
}

pub fn stop_hint() {
    println!("[nonosd] Daemon is persistent — use systemctl or kill to stop.");
}

