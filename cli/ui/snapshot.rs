// cli/src/nonosctl/ui/snapshot.rs — Sovereign Capsule Observability Graph
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Provides the entire capsule-level and mesh-state snapshot for NØN TUI and autonomous graph analyzers.

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Per-capsule expanded runtime observability metrics
#[derive(Debug, Clone)]
pub struct CapsuleMetric {
    pub capsule_id: String,
    pub pid: u32,
    pub kind: String, // Daemon, ZkMesh, Task, etc.
    pub ram_bytes: u64,
    pub cpu_pct: f32,
    pub trust_score: i16,
    pub crash_count: u32,
    pub uptime_secs: u64,
    pub zk_verified: bool,
    pub zk_latency_ms: Option<u32>,
    pub beacon_verified: bool,
    pub has_mac_spoof: bool,
    pub has_dns_mask: bool,
    pub relay_used: bool,
    pub onion_depth: u8,
    pub capsule_status: String, // Running, Idle, Terminated, etc.
    pub last_exit_code: Option<i32>,
    pub kernel_violation: Option<String>,
    pub tags: Vec<String>,
    pub sandbox_flags: Vec<String>, // e.g. ["NO_NET", "NO_FS"]
}

/// NØN-OS Telemetry Graph Root Snapshot
#[derive(Debug, Clone)]
pub struct UiSnapshot {
    // ┌────────────────────────────────┐
    // │ Aggregated Capsule Metrics    │
    // └────────────────────────────────┘
    pub capsules: Vec<CapsuleMetric>,
    pub active_capsules: usize,
    pub crashed_capsules: usize,
    pub zk_capsules: usize,
    pub total_uptime: u64, // all capsules summed uptime
    pub avg_trust_score: f32,
    pub avg_cpu_load: f32,
    pub total_ram_bytes: u64,

    // ┌────────────────────────────────┐
    // │ Sovereign Mesh Observability   │
    // └────────────────────────────────┘
    pub local_peer_id: String,
    pub mesh_peers: Vec<String>,
    pub mesh_latency_ms: HashMap<String, u32>,
    pub entropy_index: f32,
    pub trust_map: HashMap<String, i16>,
    pub zk_proof_count_global: u64,
    pub gossip_propagation_rate: f32,
    pub average_onion_depth: f32,
    pub beacon_pings: u64,

    // ┌────────────────────────────────┐
    // │ Privacy & Anonymity Metrics    │
    // └────────────────────────────────┘
    pub spoofed_macs: u32,
    pub dns_masked_nodes: u32,
    pub capsules_with_relay: u32,
    pub zk_sessions_in_last_min: u32,
    pub stealth_mode_enabled: bool,
    pub audit_anonymity_score: f32, // 0.0–1.0

    // ┌────────────────────────────────┐
    // │ Hardware & System Metrics      │
    // └────────────────────────────────┘
    pub host_uptime_secs: u64,
    pub host_cpu_arch: String,
    pub host_memory_mb: u64,
    pub verified_modules: Vec<String>, // modules cryptographically validated
    pub runtime_integrity_hash: String, // Beacon validated hash of capsule_runtime

    // ┌────────────────────────────────┐
    // │ Timestamping & Version Control│
    // └────────────────────────────────┘
    pub collected_at: DateTime<Utc>,
    pub runtime_version: String,
    pub mesh_protocol_version: String,
    pub config_hash: String,
    pub beacon_snapshot_signature: Option<String>,
}

