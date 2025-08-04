// cli/src/nonosctl/services.rs — NØN-OS Capsule Service Runtime (NØN-OS native design)

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

const SERVICE_REGISTRY: &str = "/var/nonos/runtime/services.json";
const SERVICE_BIN_PATH: &str = "/usr/lib/nonos/services/";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServiceMeta {
    pub name: String,
    pub capsule: String,
    pub auto_start: bool,
    pub status: String,
    pub pid: Option<u32>,
}

pub fn start_service(service: &str) {
    let bin = format!("{}{}", SERVICE_BIN_PATH, service);
    if !Path::new(&bin).exists() {
        println!("[nonos] service '{}' not found in runtime bin.", service);
        return;
    }

    match Command::new(&bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            update_registry(service, Some(child.id()), "running");
            println!("[nonos] service '{}' started -> PID {} [READY]", service, child.id());
        }
        Err(e) => println!("[nonos] failed to start '{}': {}", service, e),
    }
}

pub fn stop_service(service: &str) {
    if let Some(mut reg) = load_registry() {
        if let Some(meta) = reg.get_mut(service) {
            if let Some(pid) = meta.pid {
                let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
                meta.status = "stopped".into();
                meta.pid = None;
                println!("[nonos] service '{}' stopped.", service);
                save_registry(&reg);
                return;
            }
        }
    }
    println!("[nonos] service '{}' not running.", service);
}

pub fn status_service(service: &str) {
    if let Some(reg) = load_registry() {
        if let Some(meta) = reg.get(service) {
            println!("[nonos] service '{}': status={}, pid={:?}", service, meta.status, meta.pid);
        } else {
            println!("[nonos] service '{}' not found in registry.", service);
        }
    }
}

pub fn list_services() {
    if let Some(reg) = load_registry() {
        for (name, meta) in reg {
            println!("[nonos] service '{}': {} {:?}", name, meta.status, meta.pid);
        }
    } else {
        println!("[nonos] no services registered.");
    }
}

pub fn restart_service(service: &str) {
    stop_service(service);
    start_service(service);
}

pub fn enable_service(service: &str) {
    if let Some(mut reg) = load_registry() {
        if let Some(meta) = reg.get_mut(service) {
            meta.auto_start = true;
            println!("[nonos] service '{}' enabled for autostart.", service);
            save_registry(&reg);
            return;
        }
    }
    println!("[nonos] service '{}' not found in registry.", service);
}

pub fn disable_service(service: &str) {
    if let Some(mut reg) = load_registry() {
        if let Some(meta) = reg.get_mut(service) {
            meta.auto_start = false;
            println!("[nonos] service '{}' disabled for autostart.", service);
            save_registry(&reg);
            return;
        }
    }
    println!("[nonos] service '{}' not found in registry.", service);
}

fn load_registry() -> Option<HashMap<String, ServiceMeta>> {
    if let Ok(mut file) = File::open(SERVICE_REGISTRY) {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            serde_json::from_str(&contents).ok()
        } else {
            None
        }
    } else {
        None
    }
}

fn save_registry(reg: &HashMap<String, ServiceMeta>) {
    if let Ok(json) = serde_json::to_string_pretty(reg) {
        if let Ok(mut file) = File::create(SERVICE_REGISTRY) {
            let _ = file.write_all(json.as_bytes());
        }
    }
}

fn update_registry(service: &str, pid: Option<u32>, status: &str) {
    if let Some(mut reg) = load_registry() {
        if let Some(meta) = reg.get_mut(service) {
            meta.pid = pid;
            meta.status = status.to_string();
            let _ = save_registry(&reg);
        }
    }
}
