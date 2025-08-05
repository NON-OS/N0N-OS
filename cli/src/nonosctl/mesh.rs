// cli/src/nonosctl/capsule_runtime.rs — Sovereign Capsule Runtime Engine for NØN-OS
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Executes, supervises, signals, and persists sovereign capsules with full process lifecycle control

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::logging::log_event;
use crate::telemetry::log_capsule_telemetry;

const RUNTIME_STATE_DIR: &str = "/run/nonos/runtime";
const EVENT_STREAM_DIR: &str = "/var/nonos/runtime/events";
const MESH_SYNC_FILE: &str = "/var/nonos/runtime/sync_state.json";
const MAX_RESTART_ATTEMPTS: u8 = 5;
const LOG_ROTATE_SIZE: u64 = 1024 * 1024; // 1MB
const BACKOFF_BASE: u64 = 5; // seconds

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CapsuleStatus {
    Launching,
    Running,
    Idle,
    Crashed,
    Restarting,
    Terminated,
    Suspended,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CapsuleType {
    Service,
    Daemon,
    Task,
    ZkMesh,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapsuleProcess {
    pub name: String,
    pub pid: u32,
    pub status: CapsuleStatus,
    pub start_time: DateTime<Utc>,
    pub path: String,
    pub restart_attempts: u8,
    pub tags: Vec<String>,
    pub memory_limit_mb: Option<u64>,
    pub cpu_limit_pct: Option<u8>,
    pub env: Option<HashMap<String, String>>,
    pub capsule_type: CapsuleType,
    pub log_path: String,
    pub telemetry_path: String,
    pub last_error: Option<String>,
    pub last_crash_at: Option<DateTime<Utc>>,
}

pub struct CapsuleRuntime {
    pub active: Arc<Mutex<HashMap<String, CapsuleProcess>>>,
}

impl CapsuleRuntime {
    pub fn new() -> Self {
        fs::create_dir_all(RUNTIME_STATE_DIR).ok();
        fs::create_dir_all(EVENT_STREAM_DIR).ok();
        CapsuleRuntime {
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&self, name: &str, path: &str, capsule_type: CapsuleType, tags: Vec<String>, env: Option<HashMap<String, String>>) {
        let mut registry = self.active.lock().unwrap();
        let log_path = format!("/var/nonos/logs/{}.log", name);
        let telemetry_path = format!("/var/nonos/telemetry/{}.json", name);

        let mut command = Command::new(path);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(ref env_map) = env {
            for (k, v) in env_map.iter() {
                command.env(k, v);
            }
        }

        match command.spawn() {
            Ok(mut child) => {
                let capsule = CapsuleProcess {
                    name: name.to_string(),
                    pid: child.id(),
                    status: CapsuleStatus::Running,
                    start_time: Utc::now(),
                    path: path.to_string(),
                    restart_attempts: 0,
                    tags,
                    memory_limit_mb: None,
                    cpu_limit_pct: None,
                    env,
                    capsule_type,
                    log_path: log_path.clone(),
                    telemetry_path: telemetry_path.clone(),
                    last_error: None,
                    last_crash_at: None,
                };

                registry.insert(name.to_string(), capsule.clone());
                self.persist_state(&capsule);
                self.emit_event(&capsule.name, "spawned");
                log_capsule_telemetry(name, capsule.pid as i32, "spawned");
                log_event("runtime", name, "start", "capsule_runtime.rs", "capsule started");
                Self::monitor(Arc::clone(&self.active), name.to_string(), child, log_path, telemetry_path);
                Self::sync_to_mesh(Arc::clone(&self.active));
            },
            Err(e) => {
                println!("[runtime] failed to launch '{}': {}", name, e);
                log_event("runtime", name, "fail", "capsule_runtime.rs", &format!("launch failed: {}", e));
            },
        }
    }

    fn monitor(rt: Arc<Mutex<HashMap<String, CapsuleProcess>>>, name: String, mut child: Child, log_path: String, telemetry_path: String) {
        thread::spawn(move || {
            let output = child.wait_with_output();
            let mut registry = rt.lock().unwrap();
            if let Some(capsule) = registry.get_mut(&name) {
                capsule.status = CapsuleStatus::Terminated;
                capsule.restart_attempts += 1;
                capsule.last_crash_at = Some(Utc::now());
                log_event("runtime", &name, "exit", "capsule_runtime.rs", "capsule exited");

                if let Ok(out) = &output {
                    fs::write(&log_path, &out.stdout).ok();
                    fs::write(&telemetry_path, serde_json::json!({
                        "exit_code": out.status.code(),
                        "ran_at": Utc::now().to_rfc3339(),
                        "capsule": name
                    }).to_string()).ok();
                }

                if capsule.restart_attempts <= MAX_RESTART_ATTEMPTS {
                    println!("[runtime] restarting '{}'...", name);
                    capsule.status = CapsuleStatus::Restarting;
                    let backoff = Duration::from_secs(BACKOFF_BASE * capsule.restart_attempts as u64);
                    thread::sleep(backoff);
                    let cloned = capsule.clone();
                    drop(registry);
                    let runtime = CapsuleRuntime::new();
                    runtime.start(&cloned.name, &cloned.path, cloned.capsule_type.clone(), cloned.tags.clone(), cloned.env.clone());
                } else {
                    println!("[runtime] '{}' exceeded restart attempts.", name);
                    capsule.status = CapsuleStatus::Failed;
                }
            }
        });
    }

    fn emit_event(&self, name: &str, action: &str) {
        let file_path = format!("{}/{}_{}.event", EVENT_STREAM_DIR, name, Utc::now().timestamp());
        let json = serde_json::json!({
            "name": name,
            "event": action,
            "timestamp": Utc::now().to_rfc3339(),
        });
        fs::write(file_path, json.to_string()).ok();
    }

    pub fn list(&self) {
        let registry = self.active.lock().unwrap();
        for (name, proc) in registry.iter() {
            println!("- {} | PID {} | {:?} | {:?}", name, proc.pid, proc.status, proc.start_time);
        }
    }

    pub fn kill(&self, name: &str) {
        let mut registry = self.active.lock().unwrap();
        if let Some(proc) = registry.remove(name) {
            let _ = Command::new("kill").arg("-9").arg(proc.pid.to_string()).output();
            println!("[runtime] '{}' terminated.", name);
            log_event("runtime", name, "kill", "capsule_runtime.rs", "capsule killed");
            fs::remove_file(format!("{}/{}.json", RUNTIME_STATE_DIR, name)).ok();
        }
    }

    pub fn restart(name: String, path: String, capsule_type: CapsuleType, tags: Vec<String>, env: Option<HashMap<String, String>>) {
        let runtime = CapsuleRuntime::new();
        runtime.start(&name, &path, capsule_type, tags, env);
    }

    fn persist_state(&self, proc: &CapsuleProcess) {
        let path = format!("{}/{}.json", RUNTIME_STATE_DIR, proc.name);
        if let Ok(json) = serde_json::to_string_pretty(proc) {
            fs::write(path, json).ok();
        }
    }

    pub fn export_runtime_state(&self) {
        let registry = self.active.lock().unwrap();
        if let Ok(json) = serde_json::to_string_pretty(&*registry) {
            fs::write(MESH_SYNC_FILE, json).ok();
        }
    }

    fn sync_to_mesh(rt: Arc<Mutex<HashMap<String, CapsuleProcess>>>) {
        thread::spawn(move || {
            loop {
                {
                    if let Ok(json) = serde_json::to_string_pretty(&*rt.lock().unwrap()) {
                        fs::write(MESH_SYNC_FILE, json).ok();
                    }
                }
                thread::sleep(Duration::from_secs(15));
            }
        });
    }

    pub fn inspect(&self, name: &str) {
        let path = format!("{}/{}.json", RUNTIME_STATE_DIR, name);
        if Path::new(&path).exists() {
            if let Ok(mut file) = File::open(&path) {
                let mut content = String::new();
                file.read_to_string(&mut content).ok();
                println!("[inspect:{}]\n{}", name, content);
            }
        } else {
            println!("[runtime] no capsule state found for '{}'.", name);
        }
    }

    pub fn rotate_log(path: &str) {
        if let Ok(metadata) = fs::metadata(path) {
            if metadata.len() > LOG_ROTATE_SIZE {
                let _ = fs::rename(path, format!("{}.old", path));
                println!("[runtime] rotated log for '{}'.", path);
            }
        }
    }
}

