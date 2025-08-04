// cli/src/nonosctl/logging.rs — NØN-OS / Logging Engine v2.5
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Structured, signed, and decentralized log infrastructure

use std::fs::{self, OpenOptions};
use std::io::{Write, BufRead, BufReader};
use std::path::{Path, PathBuf};
use chrono::Utc;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use ed25519_dalek::{Keypair, Signature, Signer};
use rand::rngs::OsRng;

const BASE_LOG_DIR: &str = "/var/log/nonos";
const INDEX_FILE: &str = "/var/log/nonos/index.json";
const ROTATE_SIZE: u64 = 1_048_576;
const EXPORT_DIR: &str = "/var/nonos/audit/";
const SECRET_KEY: &[u8] = b"nonos-secret-key-hmac";
const LOCAL_SIGNER_ID: &str = "capsule://local-node-001";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LogKind {
    Auth,
    Capsule,
    System,
    Network,
    Telemetry,
}

impl LogKind {
    fn filename(&self) -> &'static str {
        match self {
            LogKind::Auth => "auth.log",
            LogKind::Capsule => "capsules.log",
            LogKind::System => "system.log",
            LogKind::Network => "network.log",
            LogKind::Telemetry => "telemetry.log",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub session: String,
    pub component: String,
    pub level: String,
    pub message: String,
    pub kind: LogKind,
    pub metadata: Option<LogMeta>,
    pub integrity: String,
    pub signed_by: String,
    pub detached_sig: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogMeta {
    pub user_id: Option<String>,
    pub request_id: Option<String>,
    pub capsule: Option<String>,
}

pub fn log_event(
    component: &str,
    level: &str,
    message: &str,
    kind: LogKind,
    meta: Option<LogMeta>,
    session_id: Option<String>,
) {
    let session = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = Utc::now().to_rfc3339();

    let raw = format!("{}|{}|{}|{}|{}", &timestamp, &session, &component, &level, &message);

    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET_KEY).expect("HMAC setup failed");
    mac.update(raw.as_bytes());
    let integrity = hex::encode(mac.finalize().into_bytes());

    let keypair: Keypair = Keypair::generate(&mut OsRng);
    let sig: Signature = keypair.sign(raw.as_bytes());

    let entry = LogEntry {
        timestamp,
        session,
        component: component.into(),
        level: level.into(),
        message: message.into(),
        kind: kind.clone(),
        metadata: meta,
        integrity,
        signed_by: LOCAL_SIGNER_ID.into(),
        detached_sig: hex::encode(sig.to_bytes()),
    };

    let json_line = serde_json::to_string(&entry).unwrap();
    let path = Path::new(BASE_LOG_DIR).join(kind.filename());
    fs::create_dir_all(BASE_LOG_DIR).ok();
    let _ = OpenOptions::new().create(true).append(true).open(&path)
        .and_then(|mut f| writeln!(f, "{}", json_line));

    rotate_if_needed(&path);
    update_index(&entry);
}

fn rotate_if_needed(path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > ROTATE_SIZE {
            let ts = Utc::now().format("%Y%m%d%H%M%S");
            let rotated = path.with_extension(format!("log.{}", ts));
            let _ = fs::rename(path, rotated);
        }
    }
}

fn update_index(entry: &LogEntry) {
    let mut index: Vec<LogEntry> = if Path::new(INDEX_FILE).exists() {
        let data = fs::read_to_string(INDEX_FILE).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        vec![]
    };

    index.push(entry.clone());
    if index.len() > 5000 {
        index.drain(0..(index.len() - 5000));
    }
    let _ = fs::write(INDEX_FILE, serde_json::to_string_pretty(&index).unwrap());
}

pub fn show_log(kind: LogKind, filter_level: Option<&str>, filter_component: Option<&str>, lines: usize) {
    let path = Path::new(BASE_LOG_DIR).join(kind.filename());
    if !path.exists() {
        println!("[log] no {:?} log available.", kind);
        return;
    }

    let file = fs::File::open(&path).unwrap();
    let reader = BufReader::new(file);
    let entries: Vec<_> = reader
        .lines()
        .flatten()
        .filter_map(|line| serde_json::from_str::<LogEntry>(&line).ok())
        .collect();

    let recent = &entries[entries.len().saturating_sub(lines)..];
    for entry in recent {
        if filter_level.map_or(true, |lvl| entry.level == lvl)
            && filter_component.map_or(true, |cmp| entry.component == cmp) {
            println!("[{}] [{}] ({}): {} :: signed_by {}", entry.timestamp, entry.level, entry.component, entry.message, entry.signed_by);
        }
    }
}

pub fn export_logs() {
    fs::create_dir_all(EXPORT_DIR).ok();
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let target = Path::new(EXPORT_DIR).join(format!("logbundle-{}.tar.gz", ts));
    let _ = std::process::Command::new("tar")
        .arg("czf")
        .arg(&target)
        .arg(BASE_LOG_DIR)
        .status();
    println!("[log] logs exported to {:?}", target);
}

pub fn clear_logs(kind: Option<LogKind>) {
    match kind {
        Some(k) => {
            let path = Path::new(BASE_LOG_DIR).join(k.filename());
            let _ = fs::write(path, "");
        },
        None => {
            for k in [LogKind::Auth, LogKind::System, LogKind::Capsule, LogKind::Network, LogKind::Telemetry] {
                let path = Path::new(BASE_LOG_DIR).join(k.filename());
                let _ = fs::write(path, "");
            }
        }
    }
    println!("[log] logs cleared.");
}
